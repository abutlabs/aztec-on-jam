#!/usr/bin/env bash
# #49: the settlement service's checkpoint path on a PolkaJam testnet (spikes/lib/polkajam.sh).
# Aztec v6's own released packages make three chained checkpoints (tools/aztec-gen), with 1, 2
# and 1 blobs; the service is configured with their genesis (cfg.bin). Then: the second before
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
rm -rf "$G"; (cd "$GEN" && node gen.mjs "$G" 3 "$GENESIS" 6 1000 1 8 2)
python3 - "$G" "$GENESIS" "$HERE/cfg.bin" <<'PY'
import json, struct, sys
g, genesis, cfg = sys.argv[1], int(sys.argv[2]), sys.argv[3]
archive0 = bytes.fromhex(json.load(open(f"{g}/cp1.json"))["lastArchiveRoot"][2:])
open(cfg, "wb").write(archive0 + struct.pack(">QQQ", genesis, 6, 32) + (1000).to_bytes(16, "big") + b"\xff" * 32)
def payload(tag, i, header=None):
    h = header or open(f"{g}/cp{i}.header", "rb").read()
    return bytes([1, tag]) + h + open(f"{g}/cp{i}.archive", "rb").read() + open(f"{g}/cp{i}.blobs", "rb").read()
h2 = bytearray(open(f"{g}/cp2.header", "rb").read())
bad_blobs = bytearray(h2); bad_blobs[64 + 31] ^= 1                    # blobsHash
bad_time = bytearray(h2); bad_time[192:200] = (int.from_bytes(h2[192:200], "big") + 6).to_bytes(8, "big")
cases = {1: payload(1, 2), 2: payload(2, 1), 3: payload(3, 1), 4: payload(4, 2, bytes(bad_blobs)),
         5: payload(5, 2), 6: payload(6, 2, bytes(bad_time)), 7: payload(7, 2), 8: payload(8, 3)}
for t, p in cases.items():
    open(f"{g}/p{t}.bin", "wb").write(p)
blob = bytearray(open(f"{g}/cp2.blob0", "rb").read()); blob[1000] ^= 1
open(f"{g}/cp2.blob0.bad", "wb").write(blob)
PY
BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
xs() { local i=$1 n a=(); n=$(python3 -c "import json; print(json.load(open('$G/cp$i.json'))['blobs'])"); for j in $(seq 0 $((n - 1))); do a+=(--extrinsic "@$G/cp$i.blob$j"); done; echo "${a[@]} --exports $((32 * n))"; }
pj_send 1 "$G/p1.bin" $(xs 2)
pj_send 2 "$G/p2.bin" $(xs 1)
pj_send 3 "$G/p3.bin" $(xs 1)
pj_send 4 "$G/p4.bin" $(xs 2)
pj_send 5 "$G/p5.bin" --extrinsic "@$G/cp2.blob0.bad" --extrinsic "@$G/cp2.blob1" --exports 64
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
  n=$(case $t in 2|3|8) echo 1 ;; *) echo 2 ;; esac)
  printf '| %s | %s | %s | %s | %s |\n' "${names[$t]}" "$n" "$(python3 -c "print(f'{int(\"${g:-0}\"):,}')")" "$(python3 -c "import sys; v=sys.argv[1].strip('\"'); print(int(v, 16) if v and all(c in '0123456789abcdef' for c in v) else 'none: ' + v)" "$v")" "${want[$t]}"
done
echo
for i in 1 2 3; do
  stored=$(pj_read "$(python3 -c "print(b'cp:'.hex() + ($i).to_bytes(8, 'little').hex())")" | tr -d '"' | sed 's/^0x//' | cut -c1-64)
  aztec=$(python3 -c "import json; print(json.load(open('$G/cp$i.json'))['headerHash'][2:])")
  echo "cp:$i header hash $([ "$stored" = "$aztec" ] && echo "equals" || echo "DIFFERS from") Aztec's: $aztec"
done
echo "tip:pending $(pj_read 7469703a70656e64696e67 | cut -c1-20)…"
