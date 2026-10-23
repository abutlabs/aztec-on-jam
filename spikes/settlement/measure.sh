#!/usr/bin/env bash
# #26's skeleton on a PolkaJam testnet: proofs A (1 -> 2), A again, C (5 -> 6), B (2 -> 3), and B
# with a public input altered; after each, the service's tip, proven count and last outcome.
# proofs/ holds the three proofs of #23's 2^16 every-relation circuit (each verified by bb).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
payload() { # <proof dir> [tamper]
  python3 - "$1" "${2:-}" <<'PY'
import sys
d, tamper = sys.argv[1], sys.argv[2]
vk = open(f"{d}/proof/vk", "rb").read(); pubs = bytearray(open(f"{d}/proof/public_inputs", "rb").read())
if tamper: pubs[95] ^= 1   # the third public input; previous and end unchanged
proof = open(f"{d}/proof/proof", "rb").read()
sys.stdout.write((len(vk).to_bytes(2, "little") + vk + (len(pubs) // 32).to_bytes(2, "little") + bytes(pubs) + proof).hex())
PY
}
for p in A B C; do payload "$HERE/proofs/$p" > "$O/$p.hex"; done
payload "$HERE/proofs/B" tamper > "$O/Bbad.hex"
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
int() { python3 -c "import sys; h=sys.argv[1]; print(int.from_bytes(bytes.fromhex(h),'big') if h and h != '(none)' else '-')" "$1"; }
le() { python3 -c "import sys; h=sys.argv[1]; print(int.from_bytes(bytes.fromhex(h),'little') if h and h != '(none)' else 0)" "$1"; }
echo
echo "| sent | expected | refine gas | tip after | proven | last (1 advanced, 2 stale, 0 failed) |"
echo "|---|---|---|---|---|---|"
for step in "A:advance 1 -> 2" "A:stale (previous 1, tip 2)" "C:stale (previous 5, tip 2)" "B:advance 2 -> 3" "Bbad:fails in refine"; do
  f=${step%%:*}; tag="$f-$RANDOM"
  pj_send "$tag" "$O/$f.hex"; sleep 8
  g=$(pj_refines "$tag" | awk '{print $1}' | sort -n | head -1)
  printf '| %s | %s | %s | %s | %s | %s |\n' "$f" "${step#*:}" "$(python3 -c "print(f'{${g:-0}:,}')")" \
    "$(int "$(pj_read 746970)")" "$(le "$(pj_read 70726f76656e)")" "$(pj_read 6c617374)"
done
