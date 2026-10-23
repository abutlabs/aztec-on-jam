#!/usr/bin/env python3
"""Serves the wallet page and nothing else: the page (demo-wallet `jam/`), its bundled provers
(`jam/dist`) and the compiled transfer circuit (`/circuit/transfer.json`). It holds no keys
and has no API: each browser keeps its own secret, proves its own transfers and talks to the
chain through the indexer (CORS). Decision #19's way back, taken.

  GET /                        the page (?user=<name> picks a wallet in this browser)
  GET /<file>                  the page's files, and dist/ (built by `npm run build` there)
  GET /circuit/transfer.json   circuits/transfer's compiled program, the one the service's vk matches

Env: PORT (19710), WALLET_UI (../demo-wallet/jam).
"""
import os, sys, time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
PORT = int(os.environ.get("PORT", "19710"))
UI = Path(os.environ.get("WALLET_UI", ROOT.parent / "demo-wallet" / "jam")).resolve()
CIRCUIT = ROOT / "circuits" / "transfer" / "target" / "transfer.json"
TYPES = {".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".json": "application/json",
         ".html": "text/html", ".css": "text/css", ".svg": "image/svg+xml"}


class H(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(UI), **kw)

    def guess_type(self, path):
        return TYPES.get(Path(str(path)).suffix, super().guess_type(path))

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def translate_path(self, path):
        p = path.split("?", 1)[0]
        if p == "/circuit/transfer.json":
            return str(CIRCUIT)
        if p.startswith("/node_modules/") or p.startswith("/test-fixtures/"):
            return str(UI / "__nothing__")       # never serve the dependencies' sources
        return super().translate_path(path)

    def log_message(self, fmt, *args):
        sys.stderr.write("%s wallet %s\n" % (time.strftime("%H:%M:%S"), fmt % args))


if __name__ == "__main__":
    if not (UI / "dist" / "provers.js").exists():
        sys.exit(f"wallet: {UI}/dist/provers.js is missing: run `npm install && npm run build` in {UI}")
    if not CIRCUIT.exists():
        sys.exit(f"wallet: {CIRCUIT} is missing: run scripts/bootstrap.sh (it compiles the circuits)")
    print(f"wallet: http://localhost:{PORT}/?user=A  (page {UI}; it proves in the browser)", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
