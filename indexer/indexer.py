#!/usr/bin/env python3
"""The indexer: follows the privacy and public-balance services over JIP-2 and serves what
the client and the wallet need over HTTP. Stdlib Python on jamswap's offchain modules; runs
in a container on the net (nets/net.sh indexer).

  GET  /head                      best and finalized slot
  GET  /state                     roots, leaf count, leaves, transfers, shielded, unshielded
  GET  /leaves                    the note tree's leaves in order (hex)
  GET  /nullifier/<hex>           {"spent": true|false}
  GET  /deposit/<hex>             {"amount": n} or {"amount": null}
  GET  /balance/<account-hex>     {"balance": n} on the public-balance service
  POST /submit/<service_id>       body: payload hex; ?core=N pins the core; returns the receipt
  GET  /status?package=<hex>&anchor=<hex>   JIP-2 workPackageStatus of a submission

Env: CHAIN_RPC (ws://reader:19800), CHAIN_SPEC (/shared/spec.json), PRIVACY_ID, PUBLIC_ID,
PORT (19700), POLL (seconds between state refreshes, 3).
"""
import json, os, struct, sys, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs
sys.path.insert(0, os.environ.get("OFFCHAIN", "/offchain"))
import chain  # noqa: E402

RPC = os.environ.get("CHAIN_RPC", "ws://reader:19800"); SPEC = os.environ.get("CHAIN_SPEC", "/shared/spec.json")
PRIVACY = int(os.environ["PRIVACY_ID"]); PUBLIC = int(os.environ["PUBLIC_ID"])
PORT = int(os.environ.get("PORT", "19700")); POLL = float(os.environ.get("POLL", "3"))
LOCK = threading.Lock()
STATE = {"ok": False}


def u64(b):
    return struct.unpack("<Q", b[:8])[0] if b else 0


def refresh():
    pv = chain.Jip2Chain(PRIVACY, RPC, chain_spec=SPEC)
    leaves = []                       # append-only: read only the leaves not seen yet
    while True:
        try:
            head = pv.head()
            at = head.hash
            n = u64(pv.read(b"leaves", at))
            if n < len(leaves):           # a shorter chain (a reorg or a new chain): start over
                leaves = []
            leaves = leaves + [pv.read(b"leaf:" + struct.pack("<Q", i), at).hex() for i in range(len(leaves), n)]
            s = {"ok": True, "net": os.environ.get("NET_NAME", ""), "slot": head.slot, "head": at.hex(), "privacy_id": PRIVACY, "public_id": PUBLIC,
                 "note_root": pv.read(b"root:note", at).hex(), "null_root": pv.read(b"root:null", at).hex(),
                 "leaves": list(leaves), "leaf_count": n, "transfers": u64(pv.read(b"transfers", at)),
                 "shielded": u64(pv.read(b"shielded", at)), "unshielded": u64(pv.read(b"unshielded", at)),
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
                ch = chain.Jip2Chain(None, RPC, chain_spec=SPEC)
                return self._json(200, {"best": ch.head().slot, "finalized": ch.finalized().slot})
            if parts == ["state"]:
                with LOCK:
                    return self._json(200 if STATE.get("ok") else 503, dict(STATE))
            if parts == ["leaves"]:
                with LOCK:
                    return self._json(200, STATE.get("leaves", []))
            if len(parts) == 2 and parts[0] == "nullifier":
                v = chain.Jip2Chain(PRIVACY, RPC, chain_spec=SPEC).read(b"nf:" + bytes.fromhex(parts[1]))
                return self._json(200, {"spent": bool(v)})
            if len(parts) == 2 and parts[0] == "deposit":
                v = chain.Jip2Chain(PRIVACY, RPC, chain_spec=SPEC).read(b"dep:" + bytes.fromhex(parts[1]))
                return self._json(200, {"amount": u64(v) if v else None})
            if len(parts) == 2 and parts[0] == "balance":
                v = chain.Jip2Chain(PUBLIC, RPC, chain_spec=SPEC).read(b"bal:" + bytes.fromhex(parts[1]))
                return self._json(200, {"balance": u64(v)})
            if parts == ["status"]:
                q = parse_qs(u.query); ch = chain.Jip2Chain(PRIVACY, RPC, chain_spec=SPEC)
                st = ch.package_status({"package_hash": q["package"][0], "anchor": q["anchor"][0]})
                return self._json(200, {"status": st})
            return self._json(404, {"error": "no such route", "routes": __doc__.split("\n")[5:14]})
        except Exception as e:  # noqa: BLE001
            return self._json(500, {"error": f"{type(e).__name__}: {e}"})

    def do_POST(self):
        parts = [p for p in urlparse(self.path).path.split("/") if p]
        try:
            if len(parts) == 2 and parts[0] == "submit":
                n = int(self.headers.get("Content-Length", "0")); payload = bytes.fromhex(self.rfile.read(n).decode().strip())
                ch = chain.Jip2Chain(int(parts[1]), RPC, chain_spec=SPEC)
                core = parse_qs(urlparse(self.path).query).get("core")
                if core:
                    ch.cores = [int(core[0])]     # a chain of dependent packages stays on one core
                r = ch.submit(payload)
                return self._json(200, r)
            return self._json(404, {"error": "no such route"})
        except Exception as e:  # noqa: BLE001
            return self._json(500, {"error": f"{type(e).__name__}: {e}"})

    def log_message(self, fmt, *args):
        sys.stderr.write("%s %s\n" % (time.strftime("%H:%M:%S"), fmt % args))


if __name__ == "__main__":
    threading.Thread(target=refresh, daemon=True).start()
    print(f"indexer: privacy {PRIVACY}, public-balance {PUBLIC}, JIP-2 {RPC}, http :{PORT}", flush=True)
    ThreadingHTTPServer(("0.0.0.0", PORT), H).serve_forever()
