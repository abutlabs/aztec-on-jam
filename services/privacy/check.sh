#!/usr/bin/env bash
# Build the privacy service with jam080-build and run it through lasair's in-process rig
# (bin/demo_deploy.exe: deploy, refine, accumulate) with the client's tx1 proof (out/tx1.hex,
# from client/privacy.py), then with a tampered one. Asserts, byte for byte, what accumulate wrote. Exit 0 only if all hold.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
SUB=$(cd "$HERE/../../.." && pwd)
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
DEMO=$SUB/lasair/_build/default/bin/demo_deploy.exe
OUT=${OUT:-$HERE/out}; mkdir -p "$OUT"
JAM=$("$BUILD" "$HERE" --out "$OUT" | awk '{print $1}')
echo "blob: $(wc -c < "$JAM") bytes"
PAY=$(cat "$HERE/../../out/tx1.hex")                        # client/privacy.py's first transfer
PUBS=${PAY:4:768}; PROOF=${PAY:772}; LENHEX=${PAY:0:4}
f() { echo "${PUBS:$(( $1 * 64 )):64}"; }                   # public input i as 64 hex chars
run() { (cd "$SUB/lasair" && LASAIR_REFINE_GAS=${LASAIR_REFINE_GAS:-1000000000} "$DEMO" "$JAM" "$1"); }
pass=0
expect() { # <log> <pattern> <name>
  if grep -q -- "$2" "$1"; then echo "  ok   $3"; pass=$((pass+1)); else echo "  FAIL $3 (wanted: $2)"; exit 1; fi
}

echo "== valid proof (tx1: a deposit claim paying 40 to B; the deposit is not on this fresh state, so accumulate must drop it)"
run "$LENHEX$PUBS$PROOF" > "$OUT/valid.log"
grep -E "gas used|verdict" "$OUT/valid.log" | head -3
expect "$OUT/valid.log" "verdict      : VALID" "refine accepts the proof"
expect "$OUT/valid.log" "no storage change" "accumulate drops a deposit claim with no deposit recorded"

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
