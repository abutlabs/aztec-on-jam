#!/usr/bin/env bash
# #30: how many users' proofs one refine checks. n plain UltraHonk proofs (#23's 2^16
# every-relation proofs and #26's three chained ones, about 13 KB each with key and inputs) in one
# payload, n = 1, 4, 8, 12 (JAM-NP's 200 KiB package limit allows about 15), and one batch with
# a proof tampered. On a PolkaJam testnet (spikes/lib/polkajam.sh).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
P=$HERE/../settlement/proofs
python3 - "$O" "$P" <<'PY'
import sys
out, P = sys.argv[1], sys.argv[2]
def item(d, tamper=False):
    vk = open(f"{d}/proof/vk", "rb").read(); pubs = bytearray(open(f"{d}/proof/public_inputs", "rb").read())
    if tamper: pubs[31] ^= 1
    proof = open(f"{d}/proof/proof", "rb").read()
    return len(vk).to_bytes(2, "little") + vk + (len(pubs) // 32).to_bytes(2, "little") + bytes(pubs) + len(proof).to_bytes(4, "little") + proof
proofs = [f"{P}/{x}" for x in "ABC"]
for tag, n in [(1, 1), (4, 4), (8, 8), (12, 12)]:
    body = b"".join(item(proofs[k % 3]) for k in range(n))
    open(f"{out}/p{tag}.bin", "wb").write(bytes([tag, n]) + body)
bad = b"".join(item(proofs[k % 3], tamper=(k == 3)) for k in range(4))
open(f"{out}/p99.bin", "wb").write(bytes([99, 4]) + bad)
PY
ls -la "$O"/p*.bin | awk '{print "  " $9 ": " $5 " bytes"}'
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in 1 4 8 12 99; do pj_send "$t" "$O/p$t.bin"; done
sleep 12
echo
echo "| proofs in one refine | refine gas | per proof | of a full package (5e9) | PolkaJam refine ($(pj_backend)) | verdict |"
echo "|---|---|---|---|---|---|"
for t in 1 4 8 12 99; do
  rows=$(pj_refines "$t" || true)
  [ -n "$rows" ] || { echo "| $t | refused | | | | $(pj_read "763a$(printf '%02x' $t)") |"; continue; }
  g=$(echo "$rows" | awk '{print $1}' | sort -n | head -1); n=$([ $t = 99 ] && echo 4 || echo $t)
  med=$(echo "$rows" | awk '{print $2}' | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}')
  printf '| %s | %s | %s | %s | %s s | %s |\n' "$([ $t = 99 ] && echo '4, the 4th tampered' || echo $t)" "$(python3 -c "print(f'{$g:,}')")" \
    "$(python3 -c "print(f'{$g//$n:,}')")" "$(python3 -c "print(f'{$g/5e9:.1%}')")" "$med" "$(pj_read "763a$(printf '%02x' $t)")"
done
