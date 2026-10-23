#!/usr/bin/env bash
# jamboree (../jamboree, our fork) built for Linux, for `jamboree rpc` and the indexer in
# containers on the Docker nets (their JIP-2 endpoint is not published to the host). Rebuilt
# when missing or older than ../jamboree's HEAD. Prints the binary's path.
#   scripts/jamboree-linux.sh [--check]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
OUT=$ROOT/out/jamboree-linux; BIN=$OUT/release/jamboree
rev=$(git -C "$SUB/jamboree" rev-parse --short HEAD 2>/dev/null || echo none)
if [ -x "$BIN" ] && [ "$(cat "$OUT/.rev" 2>/dev/null)" = "$rev" ]; then echo "$BIN"; exit 0; fi
[ "${1:-}" = --check ] && { echo "jamboree for Linux: not built at $rev" >&2; exit 1; }
mkdir -p "$OUT"
docker run --rm -v "$SUB/jamboree":/src -v jamboree-cargo:/usr/local/cargo/registry -v "$OUT":/out -w /src \
  -e CARGO_TARGET_DIR=/out rust:1.88-bookworm cargo build --release --locked -q >&2
echo "$rev" > "$OUT/.rev"; echo "$BIN"
