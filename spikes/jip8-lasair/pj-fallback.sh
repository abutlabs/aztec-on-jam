#!/usr/bin/env bash
# #58: on PolkaJam (no curve host calls), HostCallHooks falls back to field-wise arkworks. The verifier-profile
# service verifies #23's 2^16 and 2^24 proofs with FieldHooks (hooks 3) and with HostCallHooks
# (hooks 2) on a six-validator PolkaJam v0.8.0 testnet (spikes/lib/polkajam.sh). Each verdict must
# be valid, HostCallHooks must report both curve paths as PVM code (the output's last two octets,
# 2 and 2), and its refine gas must be within 1.1M of FieldHooks'. Then the privacy service
# (HostCallHooks since #58, so FieldHooks on PolkaJam: #55) refines out/offline's transfer, whose gas
# PolkaJam reports.
#   spikes/jip8-lasair/pj-fallback.sh        results in out/pj-fallback.txt
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
R=$HERE/../epoch-proof/out
python3 - "$O" "$R" <<'PY'
import sys
out, R = sys.argv[1], sys.argv[2]
for tag, (k, hooks) in enumerate([(16, 3), (16, 2), (24, 3), (24, 2)], 1):
    d = f"{R}/r{k}/proof"
    vk = open(f"{d}/vk", "rb").read(); pubs = open(f"{d}/public_inputs", "rb").read(); proof = open(f"{d}/proof", "rb").read()
    open(f"{out}/f{tag}.bin", "wb").write(bytes([0, tag, hooks]) + len(vk).to_bytes(2, "little") + vk
                                         + (len(pubs) // 32).to_bytes(2, "little") + pubs + proof)
PY
BLOB=$("$BUILD" "$HERE/../verifier-profile/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in 1 2 3 4; do pj_send "f$t" "$O/f$t.bin"; done
sleep 12
{
echo "| circuit | hooks | refine gas (PolkaJam) | verdict | paths (MSM, pairing) |"
echo "|---|---|---|---|---|"
for t in 1 2 3 4; do
  case $t in 1) c="2^16 FieldHooks" ;; 2) c="2^16 HostCallHooks" ;; 3) c="2^24 FieldHooks" ;; 4) c="2^24 HostCallHooks" ;; esac
  v=$(pj_read "$(printf '763a%02x' "$t")")
  python3 - "$c" "$(pj_refines "f$t" | head -1 | cut -d' ' -f1)" "$v" <<'PY'
import sys
c, gas, v = sys.argv[1], sys.argv[2], sys.argv[3].strip()
b = bytes.fromhex(v.removeprefix("0x")) if v else b""
paths = {0: "unused", 1: "host call", 2: "PVM code"}
print("| %s | %s | %s | %s | %s |" % (*c.split(" "), gas, "valid" if b[:1] == b"\x01" else "INVALID %r" % b[:4],
      ", ".join(paths.get(x, "?") for x in b[-2:]) if len(b) >= 2 else "?"))
PY
done
} | tee "$O/pj-fallback.txt"
TX=$HERE/../../out/offline/tx.hex
if [ -f "$TX" ]; then
  PB=$("$BUILD" "$HERE/../../services/privacy" --out "$O" | awk '{print $1}')
  pj_deploy "$PB"
  pj_send privacy "$TX"
  sleep 8
  echo "privacy service, one transfer's refine on PolkaJam: $(pj_refines privacy | head -1 | cut -d' ' -f1) gas" | tee -a "$O/pj-fallback.txt"
fi
