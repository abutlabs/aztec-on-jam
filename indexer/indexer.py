#!/usr/bin/env python3
"""The indexer: follows the privacy and public-balance services over JIP-2 and serves what
the clients and the wallet need over HTTP. It holds no keys and decides nothing: a public
view of the chain plus a relay for submissions (CORS open, so a browser wallet uses it).

  GET  /head                      best and finalized slot
  GET  /state                     roots, leaves, sealed notes, nullifiers (in order), counts
  GET  /leaves                    the note tree's leaves in order (hex)
  GET  /nullifier/<hex>           {"spent": true|false}
  GET  /deposit/<hex>             {"amount": n} or {"amount": null}
  GET  /balance/<account-hex>     {"balance": n} on the public-balance service
  GET  /key/<account-hex>         {"key": hex or null}: the account's directory entry
  GET  /notes?from=N              {"from": N, "notes": [hex, ...]}: sealed notes, leaf N onward
  POST /submit/<service_id>       body: payload hex; ?core=N pins the core; returns the receipt
  GET  /status?package=<hex>&anchor=<hex>   JIP-2 workPackageStatus of a submission

Two backends. On a jamboree net (JAMBOREE set to the `jamboree` binary) it reads through its
own JIP-2 client (indexer/jip2.py) and submits with `jamboree rpc submit`: nothing of jamswap.
Otherwise (the Docker nets) it uses jamswap's offchain modules, in a container on the net.

Env: CHAIN_RPC (ws://reader:19800), CHAIN_SPEC (/shared/spec.json), PRIVACY_ID, PUBLIC_ID,
PORT (19700), POLL (3 s), JAMBOREE (a jamboree binary: the jamboree backend), SUBMIT_VIA_BUILDER
(jamswap backend only: a lasair jamnp-builder URL, to submit over CE-133 to the core's
guarantors instead of JIP-2 submitWorkPackage; lasair#93).
"""
import json, os, struct, subprocess, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse, parse_qs

RPC = os.environ.get("CHAIN_RPC", "ws://reader:19800"); SPEC = os.environ.get("CHAIN_SPEC", "/shared/spec.json")
PRIVACY = int(os.environ["PRIVACY_ID"]); PUBLIC = int(os.environ["PUBLIC_ID"])
PORT = int(os.environ.get("PORT", "19700")); POLL = float(os.environ.get("POLL", "3"))
JAMBOREE = os.environ.get("JAMBOREE", "")
LOCK = threading.Lock()
STATE = {"ok": False}


def u64(b):
    return struct.unpack("<Q", b[:8])[0] if b else 0


class JamboreeBackend:
    """Our JIP-2 reader, and jamboree's Rust work-package builder for submissions."""
    def __init__(self):
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import jip2
        self.local = threading.local()
        self.jip2 = jip2

    def _c(self):
        if not hasattr(self.local, "c"):
            self.local.c = self.jip2.Chain(RPC)      # one connection per thread
        return self.local.c

    def head(self):
        return self._c().head()

    def finalized(self):
        return self._c().finalized()[0]

    def read(self, service, key, at=None):
        return self._c().read(service, key, at)

    def status(self, package, anchor):
        return self._c().package_status(package, anchor)

    def submit(self, service, payload, core):
        args = [JAMBOREE, "rpc", "--url", RPC, "--chain-spec", SPEC, "submit", str(service), "0x" + payload.hex()]
        if core is not None:
            args += ["--core", str(core)]
        r = subprocess.run(args, capture_output=True, text=True, timeout=120)
        if r.returncode != 0:
            raise RuntimeError(f"jamboree rpc submit: {(r.stderr or r.stdout).strip()[-300:]}")
        return json.loads(r.stdout.strip().splitlines()[0])


class JamswapBackend:
    """jamswap's offchain modules (the Docker nets), optionally through lasair's builder."""
    def __init__(self):
        sys.path.insert(0, os.environ.get("OFFCHAIN", "/offchain"))
        import chain
        self.chain = chain
        self.builder = os.environ.get("SUBMIT_VIA_BUILDER", "").rstrip("/")

    def _c(self, service=None):
        return self.chain.Jip2Chain(service, RPC, chain_spec=SPEC)

    def head(self):
        b = self._c().head()
        return b.slot, b.hash

    def finalized(self):
        return self._c().finalized().slot

    def read(self, service, key, at=None):
        return self._c(service).read(key, at or "best")

    def status(self, package, anchor):
        return self._c(PRIVACY).package_status({"package_hash": package, "anchor": anchor})

    def submit(self, service, payload, core):
        if self.builder:
            req = {"service_id": service, "payload_hex": payload.hex()}
            if core is not None:
                req["cores"] = [core]
            r = self.chain._http_json(self.builder + "/submit", req)
            if r.get("accepted") is False:
                raise RuntimeError(r.get("error") or "every guarantor refused")
            r.setdefault("via", "builder")
            return r
        ch = self._c(service)
        if core is not None:
            ch.cores = [core]                        # a chain of dependent packages stays on one core
        return ch.submit(payload)


B = JamboreeBackend() if JAMBOREE else JamswapBackend()


def refresh():
    leaves, sealed, nfs = [], [], []           # append-only: read only what is new
    while True:
        try:
            slot, at = B.head()
            rd = lambda key: B.read(PRIVACY, key, at)  # noqa: E731 — every read at one block
            n = u64(rd(b"leaves"))
            null_size = u64(rd(b"null:size")) or 1
            if n < len(leaves) or null_size - 1 < len(nfs):   # a shorter chain: start over
                leaves, sealed, nfs = [], [], []
            leaves = leaves + [rd(b"leaf:" + struct.pack("<Q", i)).hex() for i in range(len(leaves), n)]
            sealed = sealed + [rd(b"enc:" + struct.pack("<Q", i)).hex() for i in range(len(sealed), n)]
            nfs = nfs + [rd(b"nfl:" + struct.pack("<Q", i)).hex() for i in range(len(nfs) + 1, null_size)]
            s = {"ok": True, "net": os.environ.get("NET_NAME", ""), "slot": slot, "head": at.hex(),
                 "privacy_id": PRIVACY, "public_id": PUBLIC,
                 "note_root": rd(b"root:note").hex(), "null_root": rd(b"root:null").hex(),
                 "leaves": list(leaves), "sealed": list(sealed), "leaf_count": n,
                 "nullifiers": list(nfs), "null_size": null_size, "transfers": u64(rd(b"transfers")),
                 "shielded": u64(rd(b"shielded")), "unshielded": u64(rd(b"unshielded")),
                 "updated": time.time()}
            with LOCK:
                STATE.clear(); STATE.update(s)
        except Exception as e:  # noqa: BLE001 — keep serving the last state, say why
            with LOCK:
                STATE["error"] = f"{type(e).__name__}: {e}"
        time.sleep(POLL)


class H(BaseHTTPRequestHandler):
    def _json(self, code, obj):
        body = json.dumps(obj).encode()
        self.send_response(code); self.send_header("Content-Type", "application/json")
        self.send_header("Access-Control-Allow-Origin", "*"); self.send_header("Content-Length", str(len(body)))
        self.end_headers(); self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_response(204); self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type"); self.end_headers()

    def do_GET(self):
        u = urlparse(self.path); parts = [p for p in u.path.split("/") if p]
        try:
            if parts == ["head"]:
                return self._json(200, {"best": B.head()[0], "finalized": B.finalized()})
            if parts == ["state"]:
                with LOCK:
                    return self._json(200 if STATE.get("ok") else 503, dict(STATE))
            if parts == ["leaves"]:
                with LOCK:
                    return self._json(200, STATE.get("leaves", []))
            if parts == ["notes"]:
                start = int(parse_qs(u.query).get("from", ["0"])[0])
                with LOCK:
                    return self._json(200, {"from": start, "notes": STATE.get("sealed", [])[start:]})
            if len(parts) == 2 and parts[0] == "nullifier":
                return self._json(200, {"spent": bool(B.read(PRIVACY, b"nf:" + bytes.fromhex(parts[1])))})
            if len(parts) == 2 and parts[0] == "deposit":
                v = B.read(PRIVACY, b"dep:" + bytes.fromhex(parts[1]))
                return self._json(200, {"amount": u64(v) if v else None})
            if len(parts) == 2 and parts[0] == "key":
                v = B.read(PUBLIC, b"key:" + bytes.fromhex(parts[1]))
                return self._json(200, {"key": v.hex() if v else None})
            if len(parts) == 2 and parts[0] == "balance":
                return self._json(200, {"balance": u64(B.read(PUBLIC, b"bal:" + bytes.fromhex(parts[1])))})
            if parts == ["status"]:
                q = parse_qs(u.query)
                return self._json(200, {"status": B.status(q["package"][0], q["anchor"][0])})
            return self._json(404, {"error": "no such route", "routes": __doc__.split("\n")[5:16]})
        except Exception as e:  # noqa: BLE001
            return self._json(500, {"error": f"{type(e).__name__}: {e}"})

    def do_POST(self):
        u = urlparse(self.path); parts = [p for p in u.path.split("/") if p]
        try:
            if len(parts) == 2 and parts[0] == "submit":
                n = int(self.headers.get("Content-Length", "0"))
                payload = bytes.fromhex(self.rfile.read(n).decode().strip())
                core = parse_qs(u.query).get("core")
                return self._json(200, B.submit(int(parts[1]), payload, int(core[0]) if core else None))
            return self._json(404, {"error": "no such route"})
        except Exception as e:  # noqa: BLE001
            return self._json(500, {"error": f"{type(e).__name__}: {e}"})

    def log_message(self, fmt, *args):
        sys.stderr.write("%s %s\n" % (time.strftime("%H:%M:%S"), fmt % args))


if __name__ == "__main__":
    threading.Thread(target=refresh, daemon=True).start()
    print(f"indexer: privacy {PRIVACY}, public-balance {PUBLIC}, JIP-2 {RPC}, http :{PORT}, backend "
          f"{'jamboree (' + JAMBOREE + ')' if JAMBOREE else 'jamswap' + (' via ' + B.builder if B.builder else '')}", flush=True)
    ThreadingHTTPServer(("0.0.0.0", PORT), H).serve_forever()
