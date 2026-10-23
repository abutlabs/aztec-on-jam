#!/usr/bin/env bash
# Build the privacy service with jam080-build and run it through lasair's in-process rig
# (bin/demo_deploy.exe: deploy, refine, accumulate) with the sample proof, then with a
# tampered one. Asserts, byte for byte, what accumulate wrote. Exit 0 only if all hold.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
SUB=$(cd "$HERE/../../.." && pwd)
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
DEMO=$SUB/lasair/_build/default/bin/demo_deploy.exe
OUT=${OUT:-$HERE/out}; mkdir -p "$OUT"
JAM=$("$BUILD" "$HERE" --out "$OUT" | awk '{print $1}')
echo "blob: $(wc -c < "$JAM") bytes"
PUBS=$(xxd -p -c 1000 "$HERE/fixtures/sample.pubs" | tr -d '\n')
PROOF=$(xxd -p -c 100000 "$HERE/fixtures/sample.proof" | tr -d '\n')
LEN=$(( ${#PUBS} / 2 )); LENHEX=$(printf '%02x%02x' $((LEN & 255)) $((LEN >> 8)))
run() { (cd "$SUB/lasair" && LASAIR_REFINE_GAS=${LASAIR_REFINE_GAS:-1000000000} "$DEMO" "$JAM" "$1"); }
fail() { echo "FAIL: $*"; exit 1; }
pass=0
expect() { # <log> <pattern> <name>
  if grep -q -- "$2" "$1"; then echo "  ok   $3"; pass=$((pass+1)); else echo "  FAIL $3 (wanted: $2)"; exit 1; fi
}

echo "== valid proof"
run "$LENHEX$PUBS$PROOF" > "$OUT/valid.log"
grep -E "gas used|verdict" "$OUT/valid.log" | head -3
expect "$OUT/valid.log" "verdict      : VALID" "refine accepts the proof"
expect "$OUT/valid.log" '"root:note" = 0x00000000000000000000000000000000000000000000000000000000000003e9' "new note root 0x3e9 written"
expect "$OUT/valid.log" '"root:null" = 0x00000000000000000000000000000000000000000000000000000000000007d2' "new nullifier root 0x7d2 written"
expect "$OUT/valid.log" "0x6e663a1e95b928248aa2a64eeb6e05d80a5742a6d73691cb2666c19d3bcc8dc0a429d3 = 0x01" "nullifier 0 recorded (key nf: + hash)"
expect "$OUT/valid.log" "0x6e663a000000000000000000000000000000000000000000000000000000000000002a = 0x01" "nullifier 1 (42) recorded"
expect "$OUT/valid.log" "0x6e683a00000000000000000000000000000000000000000000000000000000000001f5 = 0x01" "note hash 0 recorded (key nh: + hash)"
expect "$OUT/valid.log" "0x6e683a00000000000000000000000000000000000000000000000000000000000001f6 = 0x01" "note hash 1 recorded"
expect "$OUT/valid.log" '"transfers" = 0x0100000000000000' "transfers = 1"

echo "== tampered proof (one byte flipped)"
BAD=$(python3 -c "p='$PROOF'; i=200; b=int(p[i:i+2],16)^1; print(p[:i]+format(b,'02x')+p[i+2:])")
run "$LENHEX$PUBS$BAD" > "$OUT/tampered.log"
grep -E "gas used|verdict" "$OUT/tampered.log" | head -2
expect "$OUT/tampered.log" "verdict      : INVALID" "refine rejects the tampered proof"
expect "$OUT/tampered.log" "no storage change" "accumulate wrote nothing"

echo "== wrong public input (new note root changed after proving)"
WRONG=$(python3 -c "p='$PUBS'; i=2*64+60; print(p[:i]+'ff'+p[i+2:])")
run "$LENHEX$WRONG$PROOF" > "$OUT/wrongpub.log"
expect "$OUT/wrongpub.log" "verdict      : INVALID" "refine rejects the altered public input"
expect "$OUT/wrongpub.log" "no storage change" "accumulate wrote nothing"
echo "PASS: $pass assertions"
