#!/usr/bin/env bash
# #47: the curve kernels' refine gas by PVM instruction class, under the GP 0.8.0 gas model,
# and the gas a widening multiply or an add-with-carry would save.
# lasair's jip8_refine with JIP8_PROFILE=1 counts every basic block the PVM charges, with the
# calls off. A block's gas is spread evenly over its instructions (the GP charges a block, not
# its instructions), so the shares are an approximation. Each rewrite saves one instruction:
# - a mul_64 and a mul_upper_u_u of the same operands in one block (a 64x64->128 product);
# - a set_lt_u right after an add_64 that compares the sum with an addend (a limb carry).
# Needs spikes/jip8-lasair/measure.sh run first (its blobs and payloads), and ../lasair built.
# Writes profile.jsonl, and prints the table in RESULTS.md.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd)
O=$ROOT/spikes/jip8-lasair/out/measure; R=$ROOT/../lasair/_build/default/bin/jip8_refine.exe
{ JIP8_PROFILE=1 JIP8_SERVICE_ID=1 "$R" "$O/verifier-profile-service.jam" "$O/pvm-bn254_pairing_check-2.bin" \
    "$O/pvm-bn254_g1_msm-128.bin" "$O/pvm-bls12_381_pairing_check-2.bin" "$O/pvm-bls12_381_g1_msm-128.bin" \
    "$O/ultrahonk-2^24-field.bin"
  JIP8_PROFILE=1 JIP8_SERVICE_ID=1 "$R" "$O/kzg-gas-service.jam" "$O/kzg-point-evaluation.bin"
} | grep '^JIP8_PROFILE ' | cut -c14- > "$HERE/profile.jsonl"
python3 - "$HERE/profile.jsonl" <<'PY'
import json, sys
cls = lambda n: ('multiply' if n.startswith('mul') else
                 'add, sub, carry' if n in ('add_64', 'sub_64', 'set_lt_u', 'add_imm_64', 'set_lt_u_imm') else
                 'load, store' if n.startswith(('load', 'store')) else 'other')
print('| Kernel | Gas | Multiply | Add, sub, carry | Load, store | Widening multiply saves | Add-with-carry saves |')
print('|---|---|---|---|---|---|---|')
for l in open(sys.argv[1]):
    d = json.loads(l); T = d['block_gas']; s = {}
    for r in d['by_instruction']:
        s[cls(r['name'])] = s.get(cls(r['name']), 0) + r['gas']
    pct = lambda x: '%.1f%%' % (100 * x / T)
    print('| %s | %.1fM | %s | %s | %s | %s | %s |' % (d['payload'].removesuffix('.bin'), T / 1e6, pct(s.get('multiply', 0)),
          pct(s.get('add, sub, carry', 0)), pct(s.get('load, store', 0)), pct(d['widening_mul_saves']), pct(d['add_with_carry_saves'])))
PY
