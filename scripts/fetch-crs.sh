#!/usr/bin/env bash
# The reference string (CRS) the browser's prover loads, fetched once into a local cache that
# client/wallet.py serves at /crs/: so the wallet page proves with no CDN in the path, and its
# first proof does not wait on 20 MB from the internet. The ranges are exactly what bb.js 5.0.0
# asks for at Barretenberg.new (2^19 compressed BN254 points, 2^17 Grumpkin points, G2), and
# each file is checked against its pinned hash.
#   scripts/fetch-crs.sh            fetch what is missing or wrong; print the cache's path
#   scripts/fetch-crs.sh --check    exit 1 unless every file is there and right
set -euo pipefail
DIR=${JAM_PRIVACY_CRS:-$HOME/.cache/jam-privacy/crs}
HOST=https://crs.aztec-cdn.foundation; FALLBACK=https://crs.aztec-labs.com
# name  byte range (empty: whole file)  sha256
FILES="g1_compressed.dat 0-16777215 1d03ebeb73e1a6d426e44a2ccc88b04125ca185ab9d1679817380502a723fa80
g2.dat - 01797bfc4de5a96f0e516a9ea4537d18786dc30cb991aca4274c95822b69c32f
grumpkin_g1_v2.dat 0-4194303 64236c9455e75aeea77a94587ea607eed2e978d2609a421d66bf10bb9698b8fd"
sha() { if command -v sha256sum >/dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1; }
mkdir -p "$DIR"; bad=0
while read -r name range want; do
  f=$DIR/$name
  [ -f "$f" ] && [ "$(sha "$f")" = "$want" ] && continue
  [ "${1:-}" = --check ] && { echo "crs: $f missing or wrong" >&2; bad=1; continue; }
  r=(); [ "$range" = - ] || r=(-r "$range")
  curl -sfL "${r[@]}" -o "$f.part" "$HOST/$name" || curl -sfL "${r[@]}" -o "$f.part" "$FALLBACK/$name"
  [ "$(sha "$f.part")" = "$want" ] || { echo "crs: $name downloaded but its hash is wrong" >&2; rm -f "$f.part"; exit 1; }
  mv "$f.part" "$f"
done <<<"$FILES"
[ $bad = 0 ] && echo "$DIR"
exit $bad
