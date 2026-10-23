#!/usr/bin/env bash
# What does Ethereum's ecrecover (the 0x01 precompile) cost in refine? Aztec's settlement checks
# a proposer's ECDSA signature per checkpoint and a committee's attestations (33 of 48) per epoch
# proof (#39). ecrecover/ is the precompile's semantics on k256 (no_std), high s accepted; host/
# checks it against geth's 5 precompile vectors and 40 signatures made by Python's
# `cryptography`, each of which must recover its own signer. Then on a PolkaJam testnet
# (spikes/lib/polkajam.sh): 1 and 33 signatures, and cases that must fail.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
V=$HERE/vectors
GETH=2006f0836b30b8f689cd1df5620a9f64421c9f84
[ -f "$V/geth-ecRecover.json" ] || curl -sfL "https://raw.githubusercontent.com/ethereum/go-ethereum/$GETH/core/vm/testdata/precompiles/ecRecover.json" -o "$V/geth-ecRecover.json"

echo "== 1. on the host"
(cd "$HERE/host" && cargo run -q --release -- "$V")

python3 - "$V" "$O" <<'PY'
import json, sys
vec, out = sys.argv[1], sys.argv[2]
rows = [l.split() for l in open(f"{vec}/inputs.txt").read().splitlines()]  # input hex, signer hex
geth = {v["Name"]: v for v in json.load(open(f"{vec}/geth-ecRecover.json"))}
def items(pairs):
    return bytes([len(pairs)]) + b"".join(bytes.fromhex(i) + bytes.fromhex(a) for i, a in pairs)
valid = (bytes.fromhex(geth["ValidKey"]["Input"]), bytes.fromhex(geth["ValidKey"]["Expected"])[12:])
bad_v = (bytes.fromhex(geth["InvalidHighV-bits-1"]["Input"]), bytes(20))
wrong = [tuple(r) for r in rows[:33]]; wrong[16] = (wrong[16][0], rows[34][1])  # a signer swapped
cases = {1: items([(valid[0].hex(), valid[1].hex())]), 2: items([tuple(r) for r in rows[:33]]),
         3: items(wrong), 4: items([(bad_v[0].hex(), bad_v[1].hex())])}
for t, b in cases.items():
    open(f"{out}/p{t}.bin", "wb").write(bytes([t]) + b)
PY

echo "== 2. on a PolkaJam testnet ($("$PJD/polkajam" --version | head -1))"
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in 1 2 3 4; do pj_send "$t" "$O/p$t.bin"; done
sleep 12
names=("" "1 signature (geth ValidKey, high s)" "33 signatures (an epoch's attestations)" "33, one signer swapped (must fail)" "geth InvalidHighV (must fail)")
echo
echo "| case | refine gas | of a tiny package (1e9) | of a full one (5e9) | PolkaJam refine ($(pj_backend), $(uname -sm)) | accumulate's verdict |"
echo "|---|---|---|---|---|---|"
for t in 1 2 3 4; do
  rows=$(pj_refines "$t" || true)
  [ -n "$rows" ] || { printf '| %s | refused | | | | %s |\n' "${names[$t]}" "$(pj_read "763a0$t")"; continue; }
  g=$(echo "$rows" | awk '{print $1}' | sort -n | head -1)
  med=$(echo "$rows" | awk '{print $2}' | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}')
  printf '| %s | %s | %s | %s | %s s of %s | %s |\n' "${names[$t]}" "$(python3 -c "print(f'{$g:,}')")" "$(python3 -c "print(f'{$g/1e9:.1%}')")" \
    "$(python3 -c "print(f'{$g/5e9:.1%}')")" "$med" "$(echo "$rows" | wc -l | tr -d ' ')" "$(pj_read "763a0$t")"
done
