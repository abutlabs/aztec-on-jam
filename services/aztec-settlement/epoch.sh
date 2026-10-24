#!/usr/bin/env bash
# #51: the settlement service's epoch path on a PolkaJam testnet. Aztec v6's released packages
# make two checkpoints in one epoch, which the service accepts. Aztec's code then assembles the
# epoch's 113 root rollup public inputs and its batched blob opening (tools/aztec-gen/epoch.mjs),
# and bb 6.0.0 proves them in the epoch proof's shape (testdata/epoch-circuit: plain UltraHonk,
# Keccak transcript, 113 free public inputs). Sent: a valid proof of public inputs with the
# wrong vk tree root (accumulate refuses), the right proof with a public input changed after
# proving (refine refuses), the right proof with a wrong blob opening (refine refuses), the
# right proof (accepted: proven tip and Outbox), and the right proof again (already proven).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out/epoch; G=$O/cps; mkdir -p "$O"
. "$HERE/../../spikes/lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
GEN=$HERE/../../tools/aztec-gen; T=$HERE/testlib.py; C=$HERE/testdata/epoch-circuit
NARGO=$HOME/.nargo/versions/nargo-1.0.0-rc.3; BB=$HOME/.bb/versions/bb-6.0.0
[ -d "$GEN/node_modules" ] || (cd "$GEN" && npm install --omit=dev --no-audit --no-fund > /dev/null)
GENESIS=$(( $(date +%s) - 600 ))
NOW_SLOT=$(( ($(date +%s) - GENESIS) / 6 ))
S1=$(( NOW_SLOT - 2 )); [ $(( (S1 + 1) % 32 )) -ne 0 ] || S1=$(( S1 - 1 ))   # S1 and S1 + 1 in one epoch
rm -rf "$G"; mkdir -p "$G"
cat > "$G/spec.json" <<JSON
{"genesis": $GENESIS, "slotSeconds": 6, "fee": 1000, "genesisArchive": null, "checkpoints": [
 {"name": "a", "parent": "genesis", "slot": $S1, "txs": 2},
 {"name": "b", "parent": "a", "slot": $((S1 + 1)), "txs": 8}]}
JSON
(cd "$GEN" && node gen.mjs "$G" "$G/spec.json")
# the circuit's vk (it doesn't depend on the inputs) goes into cfg
(cd "$C" && "$NARGO" compile --silence-warnings)
mkdir -p "$O/vk"; "$BB" write_vk -t evm-no-zk -b "$C/target/epoch_circuit.json" -o "$O/vk" > /dev/null
"$T" cfg "$G" "$GENESIS" "$HERE/cfg.bin" "$O/vk/vk"
BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
xs() { "$T" extrinsics "$G" "$1"; }
"$T" payload 1 "$G" a 0 > "$G/p1.bin"; pj_send 1 "$G/p1.bin" $(xs a)
"$T" payload 2 "$G" b 0 > "$G/p2.bin"; pj_send 2 "$G/p2.bin" $(xs b)
# the epoch's public inputs, by Aztec's code: right, and with the vk tree root wrong
prove() { # <spec.json> <out dir>: Aztec's public inputs, proved
  (cd "$GEN" && node epoch.mjs "$G" "$2/e" "$1")
  python3 -c "import json, sys; p = json.load(open(sys.argv[1])); open(sys.argv[2], 'w').write('x = \"3\"\ninputs = [' + ', '.join(f'\"{v}\"' for v in p) + ']\n')" "$2/e.pubs.json" "$C/Prover.toml"
  (cd "$C" && "$NARGO" execute --silence-warnings > /dev/null)
  "$BB" prove -t evm-no-zk -b "$C/target/epoch_circuit.json" -w "$C/target/epoch_circuit.gz" -k "$O/vk/vk" -o "$2" > "$2/prove.log" 2>&1
  "$BB" verify -t evm-no-zk -p "$2/proof" -i "$2/public_inputs" -k "$O/vk/vk" > /dev/null 2>&1 && echo "  bb verifies the proof in $2"
}
GA=$(python3 -c "import json; print(json.load(open('$G/genesis.json'))['archive'])")
for v in right wrongvk; do
  mkdir -p "$O/$v"
  root=$([ $v = right ] && echo 0x01 || echo 0x05)
  echo "{\"checkpoints\": [\"a\", \"b\"], \"previousArchive\": \"$GA\", \"previousInboxRollingHash\": \"0x00\", \"chainId\": 31337, \"version\": 1, \"vkTreeRoot\": \"$root\", \"protocolContractsHash\": \"0x02\", \"proverId\": 7}" > "$O/$v/spec.json"
  prove "$O/$v/spec.json" "$O/$v"
done
R=$O/right; W=$O/wrongvk
"$T" epoch 3 1 2 "$O/vk/vk" "$W/e.pubs.json" "$W/proof" "$W/e.batched" > "$O/p3.bin"
"$T" epoch 4 1 2 "$O/vk/vk" "$R/e.pubs.json" "$R/proof" "$R/e.batched" "pub=2:0x2a" > "$O/p4.bin"
"$T" epoch 5 1 2 "$O/vk/vk" "$R/e.pubs.json" "$R/proof" "$R/e.batched" z > "$O/p5.bin"
"$T" epoch 6 1 2 "$O/vk/vk" "$R/e.pubs.json" "$R/proof" "$R/e.batched" > "$O/p6.bin"
"$T" epoch 7 1 2 "$O/vk/vk" "$R/e.pubs.json" "$R/proof" "$R/e.batched" > "$O/p7.bin"
for t in 3 4 5 6 7; do pj_send "$t" "$O/p$t.bin"; done
sleep 12
rd() { pj_read "$1" | tr -d '"'; }
num() { python3 -c "import sys; v = sys.argv[1]; print(int(v, 16) if v and all(c in '0123456789abcdef' for c in v) else 'none: ' + v)" "$1"; }
names=("" "checkpoint a" "checkpoint b" "epoch proof, vk tree root wrong (valid proof)" "epoch proof, a public input changed after proving" "epoch proof, blob opening's z altered" "epoch proof" "epoch proof again")
want=("" 1 1 36 4 5 1 37)
echo
echo "| item | refine gas | verdict | expected |"
echo "|---|---|---|---|"
for t in 1 2 3 4 5 6 7; do
  g=$(pj_refines "$t" | awk '{print $1}' | sort -n | head -1)
  printf '| %s | %s | %s | %s |\n' "${names[$t]}" "$(python3 -c "print(f'{int(\"${g:-0}\"):,}')")" "$(num "$(rd "$(printf '763a%02x' "$t")")")" "${want[$t]}"
done
P=$(rd "$(python3 -c "print(b'tip:proven'.hex())")")
EPOCH=$(( S1 / 32 ))
OB=$(rd "$(python3 -c "print(b'outbox:'.hex() + ($EPOCH).to_bytes(8, 'little').hex())")")
python3 - "$P" "$OB" "$G" "$R/e.json" "$(rd "$(python3 -c "print(b'cp:'.hex() + (2).to_bytes(8, 'little').hex())")")" <<'PY'
import json, sys
p, ob, g, e, cp2 = sys.argv[1:6]
b = json.load(open(f"{g}/b.json"))
print(f"tip:proven checkpoint {int.from_bytes(bytes.fromhex(p[:16]), 'little')}, archive {'equals' if p[16:80] == b['archive'][2:] else 'DIFFERS from'} checkpoint b's")
print(f"outbox: out hash {'equals' if ob[:64] == b['outHash'][2:] else 'DIFFERS from'} checkpoint b's, {int.from_bytes(bytes.fromhex(ob[64:80]), 'little')} checkpoints")
print(f"cp:2 blob commitments hash {'equals' if cp2[272:336] == json.load(open(e))['blobCommitmentsHash'][2:] else 'DIFFERS from'} Aztec's BatchedBlob's")
PY
