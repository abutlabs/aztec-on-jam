#!/usr/bin/env bash
# What do EIP-4844's KZG checks cost in refine? (#24)
#   - the point evaluation (Ethereum's 0x0a precompile; Aztec's BlobLib.sol calls it once per
#     epoch proof): c-kzg-4844's verify_kzg_proof plus the versioned-hash check;
#   - verify_blob_kzg_proof, binding one blob's data to its commitment (Ethereum's consensus
#     does this for every blob; on JAM a service must).
# kzg/ is no_std Rust on zkcrypto's bls12_381; host/ checks it against c-kzg-4844's vectors
# (122 verify_kzg_proof, 29 verify_blob_kzg_proof) and geth's precompile vector first. Then on
# a PolkaJam testnet (spikes/lib/polkajam.sh): valid cases, and cases that must fail.
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
if [ ! -d "$V/blob" ]; then
  mkdir -p "$V/blob"
  gh api "repos/ethereum/c-kzg-4844/contents/tests/verify_blob_kzg_proof/kzg-mainnet?ref=$CKZG" --jq '.[].name' | while read -r n; do
    curl -sfL "https://raw.githubusercontent.com/ethereum/c-kzg-4844/$CKZG/tests/verify_blob_kzg_proof/kzg-mainnet/$n/data.yaml" -o "$V/blob/$n.yaml"
  done
fi

echo "== 1. on the host"
(cd "$HERE/host" && cargo run -q --release -- "$HERE/vectors")

# payloads, raw: [tag][0][versioned hash z y C pi] or [tag][1][n][n x (blob C pi)]
python3 - "$HERE/vectors" "$O" <<'PY'
import glob, hashlib, json, re, sys
vec, out = sys.argv[1], sys.argv[2]
def fields(path, keys):
    y = open(path).read()
    return {k: bytes.fromhex(re.search(rf"{k}: '0x([0-9a-f]*)'", y).group(1)) for k in keys}
def point(name):
    f = fields(f"{vec}/ckzg/{name}.yaml", ("commitment", "z", "y", "proof"))
    vh = b"\x01" + hashlib.sha256(f["commitment"]).digest()[1:]
    return bytes([0]) + vh + f["z"] + f["y"] + f["commitment"] + f["proof"]
def blobs(*names):
    items = b""
    for n in names:
        f = fields(f"{vec}/blob/{n}.yaml", ("blob", "commitment", "proof"))
        items += f["blob"] + f["commitment"] + f["proof"]
    return bytes([1, len(names)]) + items
correct = sorted(p.split("/")[-1][:-5] for p in glob.glob(f"{vec}/blob/verify_blob_kzg_proof_case_correct_proof_*.yaml")
                 if "point_at_infinity" not in p)
wrong = sorted(p.split("/")[-1][:-5] for p in glob.glob(f"{vec}/blob/verify_blob_kzg_proof_case_incorrect_proof_*.yaml")
               if "point_at_infinity" not in p)
geth = bytes([0]) + bytes.fromhex(json.load(open(f"{vec}/geth-pointEvaluation.json"))[0]["Input"])
bad_y = bytearray(geth); bad_y[96] ^= 1
cases = {1: geth, 2: point("verify_kzg_proof_case_correct_proof_1_2"), 3: point("verify_kzg_proof_case_correct_proof_4_4"),
         4: point("verify_kzg_proof_case_correct_proof_6_5"), 5: bytes(bad_y), 6: point("verify_kzg_proof_case_incorrect_proof_2_3"),
         7: blobs(correct[0]), 8: blobs(correct[3]), 9: blobs(wrong[0])}
for t, b in cases.items():
    open(f"{out}/p{t}.bin", "wb").write(bytes([t]) + b)
# kind 3: blobs as the work-item's extrinsics, C and pi in the payload; export 0 or 1
def ext(t, names, export):
    proofs = b""
    for i, n in enumerate(names):
        f = fields(f"{vec}/blob/{n}.yaml", ("blob", "commitment", "proof"))
        open(f"{out}/x{t}_{i}.bin", "wb").write(f["blob"])
        proofs += f["commitment"] + f["proof"]
    open(f"{out}/p{t}.bin", "wb").write(bytes([t, 3, len(names), export]) + proofs)
ext(10, correct[:1], 0); ext(11, correct[:1], 1); ext(12, correct[:3], 1); ext(13, (correct * 2)[:4], 0)
# kind 4: the batch check over extrinsic blobs; 15 has a wrong proof in the middle
def batch(t, names, bad=None):
    proofs = b""
    for i, n in enumerate(names):
        f = fields(f"{vec}/blob/{n}.yaml", ("blob", "commitment", "proof"))
        open(f"{out}/x{t}_{i}.bin", "wb").write(f["blob"])
        pi = f["proof"] if i != bad else fields(f"{vec}/blob/{wrong[0]}.yaml", ("proof",))["proof"]
        proofs += f["commitment"] + pi
    open(f"{out}/p{t}.bin", "wb").write(bytes([t, 4, len(names)]) + proofs)
batch(14, correct[:3]); batch(15, correct[:3], bad=1); batch(16, correct[:2]); batch(17, correct[:6])
# kind 5: re-check the blob case 11 exported, imported as its 32 segments; 19 with a wrong proof
f = fields(f"{vec}/blob/{correct[0]}.yaml", ("commitment", "proof"))
open(f"{out}/p18.bin", "wb").write(bytes([18, 5, 32]) + f["commitment"] + f["proof"])
w = fields(f"{vec}/blob/{wrong[0]}.yaml", ("proof",))
open(f"{out}/p19.bin", "wb").write(bytes([19, 5, 32]) + f["commitment"] + w["proof"])
PY

echo "== 2. on a PolkaJam testnet ($("$PJD/polkajam" --version | head -1))"
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in 1 2 3 4 5 6 7 8 9; do pj_send "$t" "$O/p$t.bin"; done
xargs_for() { local a=(); for f in "$O"/x"$1"_*.bin; do a+=(--extrinsic "@$f"); done; echo "${a[@]}"; }
pj_send 10 "$O/p10.bin" $(xargs_for 10)
pj_send 11 "$O/p11.bin" $(xargs_for 11) --exports 32
pj_send 12 "$O/p12.bin" $(xargs_for 12) --exports 96
pj_send 13 "$O/p13.bin" $(xargs_for 13)
pj_send 14 "$O/p14.bin" $(xargs_for 14)
pj_send 15 "$O/p15.bin" $(xargs_for 15)
pj_send 16 "$O/p16.bin" $(xargs_for 16)
pj_send 17 "$O/p17.bin" $(xargs_for 17)
# imports: the 32 segments case 11's package exported. By segments root (from its work-report,
# JIP-2 workReport) for 18 and 19; by package hash (the tagged form, which the guarantors
# resolve) for 20, same payload as 18.
sleep 12
R11=$(python3 -c "import json,sys; print(json.loads(open(sys.argv[1]).read().splitlines()[-1])['status']['Reported']['report_hash'])" "$O/submit-11.json")
pj_rpc report "$R11" > "$O/report-11.json"
ROOT11=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['segments_root'])" "$O/report-11.json")
P11=$(python3 -c "import json,sys; print(json.loads(open(sys.argv[1]).readline())['package_hash'])" "$O/submit-11.json")
by_root=(); by_pkg=(); for i in $(seq 0 31); do by_root+=(--import "root:$ROOT11:$i"); by_pkg+=(--import "pkg:$P11:$i"); done
pj_send 18 "$O/p18.bin" "${by_root[@]}"
pj_send 19 "$O/p19.bin" "${by_root[@]}"
cp "$O/p18.bin" "$O/p20.bin"; printf '\x14' | dd of="$O/p20.bin" bs=1 seek=0 count=1 conv=notrunc 2>/dev/null
pj_send 20 "$O/p20.bin" "${by_pkg[@]}"
sleep 12
echo
echo "| case | refine gas | of a tiny package (1e9) | PolkaJam refine ($(pj_backend), $(uname -sm)) | accumulate's verdict |"
echo "|---|---|---|---|---|"
names=("" "point evaluation: geth pointEvaluation1" "point evaluation: c-kzg correct_proof_1_2" "point evaluation: c-kzg correct_proof_4_4"
       "point evaluation: c-kzg correct_proof_6_5" "point evaluation: geth's, y altered (must fail)" "point evaluation: c-kzg incorrect_proof_2_3 (must fail)"
       "blob: c-kzg correct_proof, first" "blob: c-kzg correct_proof, fourth" "blob: c-kzg incorrect_proof (must fail)"
       "1 blob as an extrinsic" "1 blob as an extrinsic, exported (32 segments)" "3 blobs as extrinsics, exported (96 segments)"
       "4 blobs as extrinsics (past a tiny package's gas)" "3 blobs as extrinsics, batch-checked"
       "3 blobs, batch-checked, one wrong proof (must fail)" "2 blobs as extrinsics, batch-checked"
       "6 blobs (an Aztec checkpoint) as extrinsics, batch-checked"
       "the blob case 11 exported, imported by segments root (32 segments) and re-checked" "the same, with a wrong proof (must fail)"
       "the same as 18, imported by package hash (the tagged form)")
for t in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  rows=$(pj_refines "$t" || true)
  if [ -z "$rows" ]; then
    printf '| %s | refused (#36) | | | %s |\n' "${names[$t]}" "$(pj_read "763a$(printf '%02x' "$t")")"; continue
  fi
  g=$(echo "$rows" | awk '{print $1}' | sort -n | head -1)
  med=$(echo "$rows" | awk '{print $2}' | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}')
  printf '| %s | %s | %s | %s s of %s | %s |\n' "${names[$t]}" "$(python3 -c "print(f'{$g:,}')")" \
    "$(python3 -c "print(f'{$g/1e9:.1%}')")" "$med" "$(echo "$rows" | wc -l | tr -d ' ')" "$(pj_read "763a$(printf '%02x' "$t")")"
done
