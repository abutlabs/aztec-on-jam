#!/usr/bin/env bash
# What does EIP-4844's point evaluation (Ethereum's 0x0a precompile, which Aztec's BlobLib.sol
# calls once per epoch proof) cost in refine? kzg/ is c-kzg-4844's verify_kzg_proof in no_std
# Rust on zkcrypto's bls12_381; host/ checks it against all 122 of c-kzg's vectors and geth's
# precompile vector first. Then on a PolkaJam testnet (spikes/lib/polkajam.sh): geth's vector,
# three c-kzg proofs as precompile inputs, and two that must fail (y altered; a wrong proof).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build

# the vectors, from pinned commits of c-kzg-4844 and go-ethereum
CKZG=a751e2d92abcd63b01d1400789789f5ea289a8b0
GETH=2006f0836b30b8f689cd1df5620a9f64421c9f84
V=$HERE/vectors
if [ ! -f "$V/geth-pointEvaluation.json" ]; then
  mkdir -p "$V/ckzg"
  curl -sfL "https://raw.githubusercontent.com/ethereum/go-ethereum/$GETH/core/vm/testdata/precompiles/pointEvaluation.json" -o "$V/geth-pointEvaluation.json"
  gh api "repos/ethereum/c-kzg-4844/contents/tests/verify_kzg_proof/kzg-mainnet?ref=$CKZG" --jq '.[].name' | while read -r n; do
    curl -sfL "https://raw.githubusercontent.com/ethereum/c-kzg-4844/$CKZG/tests/verify_kzg_proof/kzg-mainnet/$n/data.yaml" -o "$V/ckzg/$n.yaml"
  done
fi

echo "== 1. on the host"
(cd "$HERE/host" && cargo run -q --release -- "$HERE/vectors")

# payloads: [tag][versioned hash][z][y][C][pi]
python3 - "$HERE/vectors" "$O" <<'PY'
import hashlib, json, re, sys
vec, out = sys.argv[1], sys.argv[2]
def ckzg(name):
    y = open(f"{vec}/ckzg/{name}.yaml").read()
    f = {k: bytes.fromhex(re.search(rf"{k}: '0x([0-9a-f]*)'", y).group(1)) for k in ("commitment", "z", "y", "proof")}
    vh = b"\x01" + hashlib.sha256(f["commitment"]).digest()[1:]
    return vh + f["z"] + f["y"] + f["commitment"] + f["proof"]
geth = bytes.fromhex(json.load(open(f"{vec}/geth-pointEvaluation.json"))[0]["Input"])
bad_y = bytearray(geth); bad_y[95] ^= 1
cases = {1: geth, 2: ckzg("verify_kzg_proof_case_correct_proof_1_2"), 3: ckzg("verify_kzg_proof_case_correct_proof_4_4"),
         4: ckzg("verify_kzg_proof_case_correct_proof_6_5"), 5: bytes(bad_y), 6: ckzg("verify_kzg_proof_case_incorrect_proof_2_3")}
for t, b in cases.items():
    open(f"{out}/p{t}.hex", "w").write((bytes([t]) + b).hex())
PY

echo "== 2. on a PolkaJam testnet ($("$PJD/polkajam" --version | head -1))"
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in 1 2 3 4 5 6; do pj_send "$t" "$O/p$t.hex"; done
sleep 12
echo
echo "| case | refine gas | of a tiny package (1e9) | PolkaJam refine ($(pj_backend), $(uname -sm)) | accumulate's verdict |"
echo "|---|---|---|---|---|"
names=("" "geth pointEvaluation1" "c-kzg correct_proof_1_2" "c-kzg correct_proof_4_4" "c-kzg correct_proof_6_5" "geth's, y altered (must fail)" "c-kzg incorrect_proof_2_3 (must fail)")
for t in 1 2 3 4 5 6; do
  read -r g s < <(pj_refines "$t" | sort -n | head -1) || true
  med=$(pj_refines "$t" | awk '{print $2}' | sort -n | awk '{a[NR]=$1} END{print (NR ? a[int((NR+1)/2)] : "")}')
  printf '| %s | %s | %s | %s s of %s | %s |\n' "${names[$t]}" "$(python3 -c "print(f'{${g:-0}:,}')")" \
    "$(python3 -c "print(f'{${g:-0}/1e9:.1%}')")" "$med" "$(pj_refines "$t" | wc -l | tr -d ' ')" "$(pj_read "763a$(printf '%02x' "$t")")"
done
