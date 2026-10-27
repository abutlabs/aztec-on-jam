#!/usr/bin/env python3
"""The curve calls' gas at chosen input lengths, from the JIP's schedules alone
(docs/jip/jip-final.md, *Gas pricing* and the common rules), written out here: no client's
code is read. Writes docs/jip/vectors/jip8-gas.jsonl, one {"call", "phi8", "gas"} a line.

The rules: n = floor(phi8 / item), at least one item for a length that is not whole items and,
for an MSM, for phi8 = 0. MSMs by table, linear between listed sizes rounded up, the listed
per-point constant past the last size. Pairing checks by base + k * item.
Python integers are unbounded, as the JIP's arithmetic is."""
import json, os

ITEM = {"bn254_g1_msm": 96, "bn254_pairing_check": 192, "bls12_381_g1_msm": 160,
        "bls12_381_pairing_check": 384, "grumpkin_g1_msm": 96}
MSM = {
    "bn254_g1_msm": ([(1, 703_667), (2, 879_660), (4, 1_135_920), (8, 3_178_140), (16, 3_178_140),
                      (32, 6_411_967), (64, 8_007_640), (128, 12_852_234), (256, 30_809_314),
                      (512, 39_471_280)], 33_836),
    "bls12_381_g1_msm": ([(1, 842_540), (2, 1_198_814), (4, 5_997_400), (8, 5_997_400),
                          (16, 14_589_460), (32, 16_341_307), (64, 30_107_254), (128, 66_119_300),
                          (256, 82_714_587), (512, 208_961_727)], 493_153),
    "grumpkin_g1_msm": ([(1, 1_091_714), (2, 1_188_054), (4, 1_604_020), (8, 2_386_260),
                         (16, 3_778_494), (32, 6_151_940), (64, 10_875_667), (128, 17_737_614),
                         (256, 31_408_427), (512, 58_925_247), (1024, 97_223_727),
                         (2048, 169_894_200), (4096, 304_263_534), (8192, 569_925_254),
                         (16384, 1_034_632_760), (32768, 1_926_735_634)], 54_450),
}
PAIR = {"bn254_pairing_check": (4_196_895, 2_030_652), "bls12_381_pairing_check": (1_827_334, 1_727_240)}


def items(call, phi8):
    n, partial = divmod(phi8, ITEM[call])
    if partial or (call in MSM and n == 0):
        n = max(n, 1)
    return n


def gas(call, phi8):
    n = items(call, phi8)
    if call in PAIR:
        base, per = PAIR[call]
        return base + per * n
    table, past = MSM[call]
    for (a, ga), (b, gb) in zip(table, table[1:]):
        if n <= a:
            return ga
        if n <= b:
            return ga + -(-(gb - ga) * (n - a) // (b - a))   # rounded up
    last, glast = table[-1]
    return glast if n <= last else glast + past * (n - last)


def lengths(call):
    item = ITEM[call]
    out = {0, 1, item - 1, item, 2 * item + 7, 1 << 32, 1 << 40}
    if call in MSM:
        sizes = [n for n, _ in MSM[call][0]]
        out |= {n * item for n in sizes}
        out |= {((a + b) // 2 + 1) * item for a, b in zip(sizes, sizes[1:]) if b - a > 1}
        out |= {(a + 1) * item for a in sizes}            # one past each: a fraction to round
        out |= {(sizes[-1] + 1) * item, (sizes[-1] + 1000) * item}
    else:
        out |= {k * item for k in (0, 1, 2, 3, 8)}
    return sorted(out)


here = os.path.dirname(os.path.abspath(__file__))
dest = os.path.join(here, "..", "..", "docs", "jip", "vectors", "jip8-gas.jsonl")
with open(dest, "w") as f:
    for call in ITEM:
        for phi8 in lengths(call):
            f.write(json.dumps({"call": call, "phi8": phi8, "gas": gas(call, phi8)}) + "\n")
print(f"wrote {dest}")
