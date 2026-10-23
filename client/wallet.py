#!/usr/bin/env python3
"""Serves the wallet page and nothing else: the page (demo-wallet `jam/`), its bundled provers
(`jam/dist`) and the compiled transfer circuit (`/circuit/transfer.json`). It holds no keys
and has no API: each browser keeps its own secret, proves its own transfers and talks to the
chain through the indexer (CORS). Decision #19's way back, taken.

  GET /                        the page (?user=<name> picks a wallet in this browser)
  GET /<file>                  the page's files, and dist/ (built by `npm run build` there)
  GET /circuit/transfer.json   circuits/transfer's compiled program, the one the service's vk matches
  GET /crs/<file>              bb.js's reference string, from the local cache scripts/fetch-crs.sh
                               fills (byte ranges honoured): no CDN on the proving path

The page is served cross-origin isolated (COOP same-origin, COEP require-corp), so bb.js gets
SharedArrayBuffer and proves on every core; the indexer's CORS answers satisfy COEP.

Env: PORT (19710), WALLET_UI (../demo-wallet/jam), JAM_PRIVACY_CRS (~/.cache/jam-privacy/crs).
"""
import os, re, sys, time
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
PORT = int(os.environ.get("PORT", "19710"))
UI = Path(os.environ.get("WALLET_UI", ROOT.parent / "demo-wallet" / "jam")).resolve()
CIRCUIT = ROOT / "circuits" / "transfer" / "target" / "transfer.json"
CRS = Path(os.environ.get("JAM_PRIVACY_CRS", Path.home() / ".cache" / "jam-privacy" / "crs"))
CRS_FILES = ("g1_compressed.dat", "g2.dat", "grumpkin_g1_v2.dat")
TYPES = {".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".json": "application/json",
         ".html": "text/html", ".css": "text/css", ".svg": "image/svg+xml"}


class H(SimpleHTTPRequestHandler):
    def __init__(self, *a, **kw):
        super().__init__(*a, directory=str(UI), **kw)

    def guess_type(self, path):
        return TYPES.get(Path(str(path)).suffix, super().guess_type(path))

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        super().end_headers()

    def do_GET(self):
        p = self.path.split("?", 1)[0]
        if p.startswith("/crs/"):
            return self.crs(p[len("/crs/"):])
        return super().do_GET()

    def crs(self, name):
        """A CRS file from the cache; a Range of bytes=a-b answers 206 with just those bytes."""
        f = CRS / name
        if name not in CRS_FILES or not f.is_file():
            return self.send_error(404, f"no {name} in {CRS}: run scripts/fetch-crs.sh")
        data = f.read_bytes(); size = len(data)
        m = re.fullmatch(r"bytes=(\d+)-(\d*)", self.headers.get("Range", "").strip())
        if m:
            a = int(m.group(1)); b = min(int(m.group(2)) if m.group(2) else size - 1, size - 1)
            if a > b:
                self.send_response(416); self.send_header("Content-Range", f"bytes */{size}"); self.end_headers(); return
            body = data[a:b + 1]
            self.send_response(206); self.send_header("Content-Range", f"bytes {a}-{b}/{size}")
        else:
            body = data
            self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream")
        self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)

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
    if not all((CRS / f).is_file() for f in CRS_FILES):
        print(f"wallet: no local CRS in {CRS} (scripts/fetch-crs.sh): the page will fetch it from Aztec's host", flush=True)
    print(f"wallet: http://localhost:{PORT}/?user=A  (page {UI}; it proves in the browser)", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), H).serve_forever()
