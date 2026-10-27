#!/usr/bin/env bash
# #59: every workload of the umbrella's prediction table (#56), measured with the curve host
# calls 101-104 off (GP 0.8.0, as on PolkaJam) and on, by lasair's in-process refine
# (lasair bin/jip8_refine: the protocol's refine gas, equal on every conforming client; the
# wall time is lasair's interpreter on this machine only). One command:
#   spikes/jip8-lasair/measure.sh        tables in out/measure.md, JSON lines in out/measure.jsonl
# Needs: ../lasair built (dune build bin/jip8_refine.exe, rust/jip8-ffi built), jamswap's
# jam080-build, and the inputs the other spikes make: spikes/epoch-proof/out (r16, r20, r24),
# out/offline (a privacy transfer), services/aztec-settlement/out/epoch (epoch.sh),
# spikes/kzg-gas/vectors, spikes/batch-verify/out/p12.bin.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
O=$HERE/out/measure; mkdir -p "$O"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
REFINE=$SUB/lasair/_build/default/bin/jip8_refine.exe
[ -x "$REFINE" ] || { echo "no $REFINE: dune build bin/jip8_refine.exe in ../lasair" >&2; exit 1; }
blob() { "$BUILD" "$1" --out "$O" | awk '{print $1}'; }

VP=$(blob "$ROOT/spikes/verifier-profile/service")
PRIV=$(blob "$ROOT/services/privacy")
SETTLE=$(blob "$ROOT/services/aztec-settlement")
KZG=$(blob "$ROOT/spikes/kzg-gas/service")
BV=$(blob "$ROOT/spikes/batch-verify/service")

python3 - "$ROOT" "$O" <<'PY'
import glob, hashlib, json, re, sys
root, out = sys.argv[1], sys.argv[2]
w = lambda name, b: open(f"{out}/{name}.bin", "wb").write(b)
# UltraHonk, Aztec's epoch-proof shape: the verifier-profile service, hooks 3 (FieldHooks) and 2
for k in (16, 20, 24):
    d = f"{root}/spikes/epoch-proof/out/r{k}/proof"
    vk, pubs, proof = (open(f"{d}/{f}", "rb").read() for f in ("vk", "public_inputs", "proof"))
    for hooks, name in ((3, "field"), (2, "calls")):
        w(f"ultrahonk-2^{k}-{name}", bytes([0, k, hooks]) + len(vk).to_bytes(2, "little") + vk
          + (len(pubs) // 32).to_bytes(2, "little") + pubs + proof)
# the calls as PVM code (kind 2: which, n), for the gas each call replaces
for which, n, name in ((1, 2, "bn254_pairing_check-2"), (0, 128, "bn254_g1_msm-128"),
                       (3, 2, "bls12_381_pairing_check-2"), (2, 128, "bls12_381_g1_msm-128")):
    w(f"pvm-{name}", bytes([2, 1, which]) + n.to_bytes(2, "little"))
# a privacy transfer
w("privacy-transfer", bytes.fromhex(open(f"{root}/out/offline/tx.hex").read().strip()))
# KZG: geth's point evaluation; one blob; batches of 2 and 6
vec = f"{root}/spikes/kzg-gas/vectors"
geth = bytes.fromhex(json.load(open(f"{vec}/geth-pointEvaluation.json"))[0]["Input"])
w("kzg-point-evaluation", bytes([1, 0]) + geth)
def fields(path, keys):
    y = open(path).read()
    return {k: bytes.fromhex(re.search(rf"{k}: '0x([0-9a-f]*)'", y).group(1)) for k in keys}
correct = sorted(p for p in glob.glob(f"{vec}/blob/verify_blob_kzg_proof_case_correct_proof_*.yaml") if "point_at_infinity" not in p)
def items(n):
    b = b""
    for p in (correct * 3)[:n]:
        f = fields(p, ("blob", "commitment", "proof")); b += f["blob"] + f["commitment"] + f["proof"]
    return b
w("kzg-blob-1", bytes([7, 1, 1]) + items(1))
w("kzg-batch-2", bytes([30, 6, 2]) + items(2))
w("kzg-batch-6", bytes([31, 6, 6]) + items(6))
# proofs a package: n 2^16 proofs, from batch-verify's 12
b = open(f"{root}/spikes/batch-verify/out/p12.bin", "rb").read()
n, i, its = b[1], 2, []
for _ in range(n):
    vl = int.from_bytes(b[i:i + 2], "little"); j = i + 2 + vl
    pn = int.from_bytes(b[j:j + 2], "little"); j += 2 + 32 * pn
    pl = int.from_bytes(b[j:j + 4], "little"); j += 4 + pl
    its.append(b[i:j]); i = j
for k in (1, 20, 21, 104):
    w(f"proofs-{k}", bytes([k % 256, k]) + b"".join(its[m % n] for m in range(k)))
PY

run() { # <blob> <payload names...>
  local blob=$1; shift
  JIP8_SERVICE_ID=1 "$REFINE" "$blob" $(for p in "$@"; do echo "$O/$p.bin"; done) | grep '^JIP8_REFINE ' | cut -c13-
}
{
  run "$VP" ultrahonk-2^16-field ultrahonk-2^16-calls ultrahonk-2^20-field ultrahonk-2^20-calls \
    ultrahonk-2^24-field ultrahonk-2^24-calls pvm-bn254_pairing_check-2 pvm-bn254_g1_msm-128 \
    pvm-bls12_381_pairing_check-2 pvm-bls12_381_g1_msm-128
  run "$PRIV" privacy-transfer
  run "$KZG" kzg-point-evaluation kzg-blob-1 kzg-batch-2 kzg-batch-6
  run "$BV" proofs-1 proofs-20 proofs-21 proofs-104
  [ -f "$ROOT/services/aztec-settlement/out/epoch/p6.bin" ] && \
    JIP8_SERVICE_ID=1 "$REFINE" "$SETTLE" "$ROOT/services/aztec-settlement/out/epoch/p6.bin" | grep '^JIP8_REFINE ' | cut -c13- | sed 's/"p6.bin"/"settlement-epoch-proof"/'
} > "$O/measure.jsonl"

python3 - "$O/measure.jsonl" > "$O/measure.md" <<'PY'
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1])]
g = lambda r, k: int(r[k]["gas"]) if r[k] else None
ms = lambda r, k: r[k]["ns"] / 1e6 if r[k] else None
print("| workload | calls off: gas | calls on: gas | saving | lasair wall, off / on (this machine) |")
print("|---|---|---|---|---|")
for r in rows:
    off, on = g(r, "off"), g(r, "on")
    sav = f"{off / on:.1f}x" if off and on and on < off else ""
    cell = lambda v: f"{v:,}" if v else "out of gas (5e9)"
    wall = lambda k: f"{ms(r, k):.0f}" if r[k] else "-"
    print(f"| {r['payload'].removesuffix('.bin')} | {cell(off)} | {cell(on)} | {sav} | {wall('off')} / {wall('on')} ms |")
PY
cat "$O/measure.md"
