#!/usr/bin/env python3
"""The wallet's local backend: serves the wallet page (demo-wallet fork, `jam/`) and turns
its buttons into proofs and work-packages. Proves with client/privacy.py (nargo + bb on this
machine), submits through the indexer, and answers only when the chain shows the change.

  GET  /                      the page (WALLET_UI, default ../demo-wallet/jam)
  GET  /api/state?user=A      public and private balance, notes, the chain's view
  POST /api/mint     {"user", "amount"}            public faucet (dev chain)
  POST /api/shield   {"user", "amount"}            public -> a deposit -> a private note
  POST /api/send     {"user", "to", "amount"}      private -> private
  POST /api/unshield {"user", "amount"}            private -> public

Every key lives in this one process, one per user name (demo; see decision #19). Env:
INDEXER (http://localhost:19700), PORT (19710), WALLET_STATE (out/wallet-state.json).
"""
import json, os, shutil, struct, sys, threading, time, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import privacy  # noqa: E402

INDEXER = os.environ.get("INDEXER", "http://localhost:19700")
PORT = int(os.environ.get("PORT", "19710"))
UI = Path(os.environ.get("WALLET_UI", ROOT.parent / "demo-wallet" / "jam"))
privacy.STATE = Path(os.environ.get("WALLET_STATE", ROOT / "out" / "wallet-state.json"))
ACCOUNTS = {"A": 1, "B": 2, "C": 3}        # public accounts, as small field elements
LOCK = threading.Lock()                     # one proof at a time: the tree has one writer here
WAIT_S, TRIES = 36, 3                       # lasair#93: a Ready package may never accumulate


class WalletError(Exception):
    pass


def http(method, path, body=None, timeout=60):
    data = None if body is None else (body if isinstance(body, bytes) else body.encode())
    req = urllib.request.Request(INDEXER + path, data=data, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        raise WalletError(f"indexer {path}: {e.code} {e.read()[:200]!r}") from e
    except OSError as e:
        raise WalletError(f"indexer unreachable at {INDEXER}: {e}") from e


def chain_state():
    return http("GET", "/state")


def ids():
    s = chain_state()
    return s["privacy_id"], s["public_id"]


def account_hex(user):
    if user not in ACCOUNTS:
        raise WalletError(f"unknown user {user!r}; this demo knows {', '.join(ACCOUNTS)}")
    return f"{ACCOUNTS[user]:064x}"


def public_balance(user):
    return http("GET", f"/balance/{account_hex(user)}")["balance"]


def submit_until(service, payload, done, label, steps):
    """Send `payload` to `service` until `done()` is true; resend a package that never lands."""
    for attempt in range(1, TRIES + 1):
        t0 = time.time()
        r = http("POST", f"/submit/{service}", payload.hex(), timeout=120)
        steps.append(f"{label}: package {r['package_hash'][:12]}… on core {r['core']}"
                     + (f" (resend {attempt - 1})" if attempt > 1 else ""))
        while time.time() - t0 < WAIT_S:
            if done():
                steps.append(f"{label}: on chain after {time.time() - t0:.0f} s")
                return
            time.sleep(2)
        steps.append(f"{label}: not on chain after {WAIT_S} s")
    raise WalletError(f"{label}: never reached the chain after {TRIES} sends")


def sync():
    privacy.sync(chain_state())


def prove_and_land(steps, label, service, *args):
    """Sync, prove, submit until the new root is on chain. If any of it fails the note state
    goes back to what it was, so no note is left marked spent by a transfer that never landed."""
    snap = privacy.STATE.with_suffix(".bak")
    if privacy.STATE.exists():
        shutil.copy(privacy.STATE, snap)
    try:
        sync()
        t0 = time.time()
        r = privacy.transfer(*args)
        steps.append(f"{label}: proof of {len(r['payload'])} bytes in {time.time() - t0:.1f} s")
        root = f"{r['fields'][2]:064x}"
        submit_until(service, r["payload"], lambda: chain_state().get("note_root") == root, label, steps)
    except Exception:
        if snap.exists():
            shutil.copy(snap, privacy.STATE)
        raise


def act_mint(user, amount, steps):
    _, pb = ids(); before = public_balance(user)
    payload = bytes.fromhex("00" + account_hex(user)) + struct.pack("<Q", amount) + os.urandom(32)
    submit_until(pb, payload, lambda: public_balance(user) == before + amount, "mint", steps)


def act_shield(user, amount, steps):
    pv, pb = ids(); before = public_balance(user)
    if before < amount:
        raise WalletError(f"{user} holds {before} publicly, not {amount}")
    note = privacy.deposit(user, amount)
    c = f"{note['hash']:064x}"
    steps.append(f"deposit note: commitment {c[:12]}…")
    payload = (bytes.fromhex("02" + account_hex(user)) + struct.pack("<Q", amount)
               + struct.pack("<I", pv) + bytes.fromhex(c))
    try:
        submit_until(pb, payload, lambda: http("GET", f"/deposit/{c}")["amount"] == amount, "shield", steps)
    except WalletError:
        st = privacy.load(); st["notes"][note["id"]]["spent"] = True; privacy.save(st)   # no deposit: no note
        raise
    # claim it: spend the deposit into an ordinary note in the tree. If this fails the deposit
    # note stays, unclaimed; the next send or unshield claims it (it goes in the first slot).
    prove_and_land(steps, "claim", pv, user, f"deposit:{note['id']}", "-", user, amount, 0, 0, 0)


def act_send(user, to, amount, steps):
    pv, _ = ids()
    if to not in ACCOUNTS or to == user:
        raise WalletError(f"send to someone else: one of {', '.join(u for u in ACCOUNTS if u != user)}")
    sync()
    a, b = privacy.select_inputs(user, amount)
    prove_and_land(steps, "send", pv, user, a, b, to, amount, 0, 0, 0)


def act_unshield(user, amount, steps):
    pv, pb = ids()
    sync()
    a, b = privacy.select_inputs(user, amount)
    prove_and_land(steps, "unshield", pv, user, a, b, user, 0, amount, ACCOUNTS[user], pb)


ACTIONS = {"mint": act_mint, "shield": act_shield, "send": act_send, "unshield": act_unshield}


def state_for(user):
    s = chain_state()
    notes = [{"id": n["id"], "value": n["value"], "kind": n["kind"], "leaf": n.get("leaf")}
             for n in privacy.unspent(user)]
    return {"user": user, "account": ACCOUNTS.get(user), "public": public_balance(user),
            "private": sum(n["value"] for n in notes), "notes": notes,
            "chain": {k: s.get(k) for k in ("slot", "note_root", "null_root", "leaf_count", "transfers",
                                             "shielded", "unshielded", "privacy_id", "public_id")}}


class H(BaseHTTPRequestHandler):
    def _send(self, code, body, ctype="application/json"):
        data = body if isinstance(body, bytes) else json.dumps(body).encode()
        self.send_response(code); self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(data))); self.send_header("Cache-Control", "no-store")
        self.end_headers(); self.wfile.write(data)

    def do_GET(self):
        u = urlparse(self.path)
        if u.path == "/api/state":
            user = parse_qs(u.query).get("user", ["A"])[0]
            try:
                with LOCK:
                    return self._send(200, state_for(user))
            except (WalletError, privacy.ClientError) as e:
                return self._send(503, {"error": str(e)})
        name = "index.html" if u.path in ("/", "") else u.path.lstrip("/")
        f = (UI / name).resolve()
        if UI.resolve() not in f.parents or not f.is_file():
            return self._send(404, {"error": "not found"})
        ctype = {".html": "text/html", ".js": "text/javascript", ".css": "text/css",
                 ".svg": "image/svg+xml"}.get(f.suffix, "application/octet-stream")
        return self._send(200, f.read_bytes(), ctype + "; charset=utf-8")

    def do_POST(self):
        name = urlparse(self.path).path.removeprefix("/api/")
        if name not in ACTIONS:
            return self._send(404, {"error": f"no action {name}"})
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))) or b"{}")
            user, amount = body["user"], int(body["amount"])
            if amount <= 0:
                raise WalletError("the amount must be positive")
            args = (user, body["to"], amount) if name == "send" else (user, amount)
        except (KeyError, ValueError, json.JSONDecodeError) as e:
            return self._send(400, {"error": f"bad request: {e}"})
        except WalletError as e:
            return self._send(400, {"error": str(e)})
        steps, t0 = [], time.time()
        try:
            with LOCK:
                ACTIONS[name](*args, steps)
                out = state_for(user)
            out.update(ok=True, steps=steps, seconds=round(time.time() - t0, 1))
            return self._send(200, out)
        except (WalletError, privacy.ClientError) as e:
            return self._send(409, {"ok": False, "error": str(e), "steps": steps})

    def log_message(self, fmt, *args):
        sys.stderr.write("%s wallet %s\n" % (time.strftime("%H:%M:%S"), fmt % args))


if __name__ == "__main__":
    privacy.STATE.parent.mkdir(parents=True, exist_ok=True)
    print(f"wallet: http://localhost:{PORT}/?user=A  (indexer {INDEXER}, page {UI})", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
