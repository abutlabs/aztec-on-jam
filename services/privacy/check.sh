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
# with LTO the verifier is compiled out when the embedded vk cannot match the service's public
# inputs (a stale fixtures/transfer.vk): the blob shrinks to ~40 KB and every proof is refused
[ "$(wc -c < "$JAM")" -gt 500000 ] || { echo "FAIL: the blob is too small to hold the verifier (stale fixtures/transfer.vk?)"; exit 1; }
PAY=$(cat "$HERE/../../out/tx1.hex")                        # client/privacy.py's first transfer
# out/tx-later.hex, if present: a later transfer of the same client (its trees are not empty)
# variants of the payload [pubs_len][pubs][proof_len][proof][sealed notes], one byte changed
variant() { python3 - "$PAY" "$1" <<'PYV'
import sys
p = bytearray(bytes.fromhex(sys.argv[1])); n = int.from_bytes(p[:2], "little"); m = int.from_bytes(p[2 + n:4 + n], "little")
where = {"proof": 4 + n + 100, "pub": 2 + 3 * 32 + 30, "log": 4 + n + m + 40}[sys.argv[2]]
p[where] ^= 1; print(p.hex())
PYV
}
run() { (cd "$SUB/lasair" && LASAIR_REFINE_GAS=${LASAIR_REFINE_GAS:-1000000000} "$DEMO" "$JAM" "$1"); }
pass=0
expect() { # <log> <pattern> <name>
  if grep -q -- "$2" "$1"; then echo "  ok   $3"; pass=$((pass+1)); else echo "  FAIL $3 (wanted: $2)"; exit 1; fi
}

echo "== valid proof (tx1: a deposit claim paying 40 to B; the deposit is not on this fresh state, so accumulate must drop it)"
run "$PAY" > "$OUT/valid.log"
grep -E "gas used|verdict" "$OUT/valid.log" | head -3
expect "$OUT/valid.log" "verdict      : VALID" "refine accepts the proof"
expect "$OUT/valid.log" "no storage change" "accumulate drops a deposit claim with no deposit recorded"

echo "== tampered proof (one byte flipped)"
run "$(variant proof)" > "$OUT/tampered.log"
grep -E "gas used|verdict" "$OUT/tampered.log" | head -2
expect "$OUT/tampered.log" "verdict      : INVALID" "refine rejects the tampered proof"
expect "$OUT/tampered.log" "no storage change" "accumulate wrote nothing"

echo "== wrong public input (new note root changed after proving)"
run "$(variant pub)" > "$OUT/wrongpub.log"
expect "$OUT/wrongpub.log" "verdict      : INVALID" "refine rejects the altered public input"
expect "$OUT/wrongpub.log" "no storage change" "accumulate wrote nothing"
echo "== a sealed note changed after proving (the logs no longer match logs_hash)"
run "$(variant log)" > "$OUT/wronglog.log"
expect "$OUT/wronglog.log" "verdict      : INVALID" "refine rejects a swapped sealed note"
expect "$OUT/wronglog.log" "no storage change" "accumulate wrote nothing"
echo "== a valid proof that starts from a tree of its own making (a non-empty tree on a fresh chain)"
[ -f "$HERE/../../out/tx-later.hex" ] && {
  run "$(cat "$HERE/../../out/tx-later.hex")" > "$OUT/genesis.log"
  expect "$OUT/genesis.log" "verdict      : VALID" "refine accepts the proof itself"
  expect "$OUT/genesis.log" "no storage change" "accumulate refuses: a new chain starts from the empty trees"
}
echo "PASS: $pass assertions"
