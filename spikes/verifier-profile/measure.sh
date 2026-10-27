#!/usr/bin/env bash
# #44: how much of an UltraHonk verification would JIP-8's curve calls replace? #23's
# every-relation proofs (plain UltraHonk, Keccak, as Aztec's epoch proof) at 2^16, 2^20 and
# 2^24, each verified twice in refine on a PolkaJam testnet (spikes/lib/polkajam.sh): with the
# default `()` hooks, as every other spike runs, and with `Gauge`, which meters its own gas per
# curve operation (service/src/lib.rs). Needs spikes/epoch-proof/measure.sh's out/r16, r20, r24.
# Then #45's PVM baselines: bn254_g1_msm and bn254_pairing_check as PVM code, end to end.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
R=$HERE/../epoch-proof/out
python3 - "$O" "$R" <<'PY'
import sys
out, R = sys.argv[1], sys.argv[2]
for tag, (k, hooks) in enumerate([(16, 0), (16, 1), (20, 0), (20, 1), (24, 0), (24, 1)], 1):
    d = f"{R}/r{k}/proof"
    vk = open(f"{d}/vk", "rb").read(); pubs = open(f"{d}/public_inputs", "rb").read(); proof = open(f"{d}/proof", "rb").read()
    body = bytes([0, tag, hooks]) + len(vk).to_bytes(2, "little") + vk + (len(pubs) // 32).to_bytes(2, "little") + pubs + proof
    open(f"{out}/p{tag}.bin", "wb").write(body)
for tag, (which, n) in enumerate([(0, 1), (0, 16), (0, 64), (0, 128), (1, 1), (1, 2), (1, 4), (1, 8)], 7):
    open(f"{out}/p{tag}.bin", "wb").write(bytes([2, tag, which]) + n.to_bytes(2, "little"))
# call 105 as PVM code (lasair#130), for lasair's bin/jip8_bench (p23..p26) and, past a package's
# gas, bin/jip8_refine with JIP8_GAS_LIMIT (p27, 2^15 points, the size of Aztec's IPA)
for tag, n in zip(range(23, 28), (1, 16, 64, 128, 32768)):
    open(f"{out}/p{tag}.bin", "wb").write(bytes([2, tag, 4]) + n.to_bytes(2, "little"))
PY
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in $(seq 1 14); do pj_send "$t" "$O/p$t.bin"; done
sleep 12
echo
echo "| circuit | hooks | refine gas | verdict | verify | MSM G1 (calls, points) | mul G1 (calls) | Miller loop (pairs) | final exp | rest of verify |"
echo "|---|---|---|---|---|---|---|---|---|---|"
for t in 1 2 3 4 5 6; do
  k=$((16 + 4 * ((t - 1) / 2))); h=$([ $((t % 2)) = 1 ] && echo "()" || echo Gauge)
  g=$(pj_refines "$t" | awk '{print $1}' | sort -n | head -1)
  v=$(pj_read "$(printf '763a%02x' "$t")")
  python3 - "$k" "$h" "${g:-0}" "$v" <<'PY'
import sys
k, h, g, v = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
b = bytes.fromhex(v.removeprefix("0x")) if v.startswith("0x") or all(c in "0123456789abcdef" for c in v) else b""
f = lambda x: f"{x:,}"
if len(b) < 1 + 8 * 13:
    print(f"| 2^{k} | {h} | {f(g)} | {v} | | | | | | |"); sys.exit()
ok = b[0]; u = [int.from_bytes(b[1 + 8 * i:9 + 8 * i], "little") for i in range(13)]
verify, m = u[0], [u[1 + 3 * i:4 + 3 * i] for i in range(4)]
rest = verify - sum(r[0] for r in m)
cell = lambda r, pts=True: f"{f(r[0])} ({r[1]}, {r[2]})" if pts else f"{f(r[0])} ({r[1]})"
print(f"| 2^{k} | {h} | {f(g)} | {ok:02x} | {f(verify)} | {cell(m[0])} | {cell(m[1], False)} | {cell(m[2])} | {f(m[3][0])} | {f(rest)} |")
PY
done
echo
echo "| call, as PVM code | n | refine gas | metered: decode, check, compute |"
echo "|---|---|---|---|"
for t in $(seq 7 14); do
  case $t in 7) c=bn254_g1_msm n=1 ;; 8) c=bn254_g1_msm n=16 ;; 9) c=bn254_g1_msm n=64 ;; 10) c=bn254_g1_msm n=128 ;;
             11) c=bn254_pairing_check n=1 ;; 12) c=bn254_pairing_check n=2 ;; 13) c=bn254_pairing_check n=4 ;; 14) c=bn254_pairing_check n=8 ;; esac
  g=$(pj_refines "$t" | awk '{print $1}' | sort -n | head -1)
  v=$(pj_read "$(printf '763a%02x' "$t")")
  python3 -c "import sys; b=bytes.fromhex(sys.argv[4].removeprefix('0x')); print(f'| {sys.argv[1]} | {sys.argv[2]} | {int(sys.argv[3] or 0):,} | ' + (f'{int.from_bytes(b[1:9], \"little\"):,} |' if len(b) >= 9 and b[0] == 1 else f'{sys.argv[4]} |'))" "$c" "$n" "$g" "$v"
done
