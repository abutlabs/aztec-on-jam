#!/usr/bin/env bash
# #49: the settlement service's checkpoint path on a PolkaJam testnet (spikes/lib/polkajam.sh).
# Aztec v6's own released packages make three chained checkpoints (tools/aztec-gen), with 1, 2
# and 1 blobs (8 random transactions may fit in one); the service is configured with their genesis (cfg.bin). Then: the second before
# the first (refused), the first (accepted), the first again (refused), the second with its
# blobsHash altered, a blob altered, and its timestamp altered (all refused), the second and
# the third (accepted). Each stored checkpoint's header hash must be Aztec's.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; G=$O/cps; mkdir -p "$O"
. "$HERE/../../spikes/lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
GEN=$HERE/../../tools/aztec-gen
[ -d "$GEN/node_modules" ] || (cd "$GEN" && npm install --omit=dev --no-audit --no-fund > /dev/null)
GENESIS=$(( $(date +%s) - 3600 ))   # an hour ago, so slots 1-3 are past
T=$HERE/testlib.py
rm -rf "$G"; mkdir -p "$G"
cat > "$G/spec.json" <<JSON
{"genesis": $GENESIS, "slotSeconds": 6, "fee": 1000, "genesisArchive": null, "checkpoints": [
 {"name": "cp1", "parent": "genesis", "slot": 1, "txs": 1},
 {"name": "cp2", "parent": "cp1", "slot": 2, "txs": 8},
 {"name": "cp3", "parent": "cp2", "slot": 3, "txs": 2}]}
JSON
(cd "$GEN" && node gen.mjs "$G" "$G/spec.json")
"$T" cfg "$G" "$GENESIS" "$HERE/cfg.bin"
"$T" payload 1 "$G" cp2 0 > "$G/p1.bin"
"$T" payload 2 "$G" cp1 0 > "$G/p2.bin"
"$T" payload 3 "$G" cp1 0 > "$G/p3.bin"
"$T" payload 4 "$G" cp2 0 blobsHash > "$G/p4.bin"
"$T" payload 5 "$G" cp2 0 > "$G/p5.bin"
"$T" payload 6 "$G" cp2 0 timestamp > "$G/p6.bin"
"$T" payload 7 "$G" cp2 0 > "$G/p7.bin"
"$T" payload 8 "$G" cp3 0 > "$G/p8.bin"
python3 -c "import sys; b = bytearray(open(sys.argv[1], 'rb').read()); b[1000] ^= 1; open(sys.argv[1] + '.bad', 'wb').write(b)" "$G/cp2.blob0"
BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
xs() { "$T" extrinsics "$G" "cp$1"; }
pj_send 1 "$G/p1.bin" $(xs 2)
pj_send 2 "$G/p2.bin" $(xs 1)
pj_send 3 "$G/p3.bin" $(xs 1)
pj_send 4 "$G/p4.bin" $(xs 2)
pj_send 5 "$G/p5.bin" $(xs 2 | sed "s#@$G/cp2.blob0 #@$G/cp2.blob0.bad #")   # blob 0 altered
pj_send 6 "$G/p6.bin" $(xs 2)
pj_send 7 "$G/p7.bin" $(xs 2)
pj_send 8 "$G/p8.bin" $(xs 3)
sleep 12
names=("" "checkpoint 2 before 1" "checkpoint 1" "checkpoint 1 again" "checkpoint 2, blobsHash altered" "checkpoint 2, a blob altered" "checkpoint 2, timestamp altered" "checkpoint 2" "checkpoint 3")
want=("" 10 1 10 2 3 13 1 1)
echo
echo "| case | blobs | refine gas | verdict | expected |"
echo "|---|---|---|---|---|"
for t in $(seq 1 8); do
  g=$(pj_refines "$t" | awk '{print $1}' | sort -n | head -1)
  v=$(pj_read "$(printf '763a%02x' "$t")")
  n=$(python3 -c "import json; print(json.load(open('$G/cp' + {1: '2', 2: '1', 3: '1', 8: '3'}.get($t, '2') + '.json'))['blobs'])")
  printf '| %s | %s | %s | %s | %s |\n' "${names[$t]}" "$n" "$(python3 -c "print(f'{int(\"${g:-0}\"):,}')")" "$(python3 -c "import sys; v=sys.argv[1].strip('\"'); print(int(v, 16) if v and all(c in '0123456789abcdef' for c in v) else 'none: ' + v)" "$v")" "${want[$t]}"
done
echo
for i in 1 2 3; do
  stored=$(pj_read "$(python3 -c "print(b'cp:'.hex() + ($i).to_bytes(8, 'little').hex())")" | tr -d '"' | sed 's/^0x//' | cut -c1-64)
  aztec=$(python3 -c "import json; print(json.load(open('$G/cp$i.json'))['headerHash'][2:])")
  echo "cp:$i header hash $([ "$stored" = "$aztec" ] && echo "equals" || echo "DIFFERS from") Aztec's: $aztec"
done
echo "tip:pending $(pj_read 7469703a70656e64696e67 | cut -c1-20)…"
