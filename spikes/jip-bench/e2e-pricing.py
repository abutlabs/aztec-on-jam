#!/usr/bin/env python3
"""#45: gas for JIP-8's core calls from e2e's end-to-end timings (stdin, its "E2E" lines).

The invariant (the final JIP's *Gas pricing*): a call charges at least what the GP's own
sizing charges for the same time, 5e9 gas for 1.5 s of one core (discussion.tex:77, the
conservative end of 25-50% of a 6 s slot), so R_GP = 10/3 gas/ns, times a margin m = 2 for slower
clients and hardware. t is the reference implementation's p99, the worst over the input cases at
that size, raised to the largest at any smaller size so that gas never falls as inputs grow.
MSMs are priced by a table, interpolated; pairing checks by base + k * item, the
tightest such line above every measured k.
"""
import collections, math, sys

R_GP, M = 10 / 3, 2
REF = {"bn254_g1_msm": "arkworks", "bn254_pairing_check": "arkworks",
       "bls12_381_g1_msm": "blst", "bls12_381_pairing_check": "blst"}
worst = collections.defaultdict(lambda: (0.0, ""))
other = collections.defaultdict(float)
for line in sys.stdin:
    if not line.startswith("E2E "):
        continue
    _, call, impl, case, n, med, p99, mx = line.split()
    key = (call, int(n))
    if impl == REF[call]:
        if float(p99) > worst[key][0]:
            worst[key] = (float(p99), case.replace("_", " "))
    else:
        other[key] = max(other[key], float(p99))
gas = lambda ns: math.ceil(ns * R_GP * M)
fmt = lambda x: f"{x:,}"
for call in REF:
    ns = sorted(n for c, n in worst if c == call)
    if not ns:
        continue
    print(f"\n#### `{call}` (reference {REF[call]})\n")
    print("| n | reference p99, worst case | worst case | other library's worst p99 | gas |")
    print("|---|---|---|---|---|")
    g, top = {}, 0
    for n in ns:
        t, case = worst[(call, n)]
        top = g[n] = max(top, gas(t))
        print(f"| {n} | {t / 1e3:,.1f} µs | {case} | {other[(call, n)] / 1e3:,.1f} µs | {fmt(g[n])} |")
    if "pairing" in call:
        item = max(math.ceil((g[k] - g[ns[0]]) / (k - ns[0])) for k in ns[1:])
        base = max(g[k] - k * item for k in ns)
        print(f"\nGas = {fmt(base)} + k × {fmt(item)}, at or above every measured k.")
    else:
        lo, hi = ns[-2], ns[-1]
        slope = math.ceil((g[hi] - g[lo]) / (hi - lo))
        print(f"\nGas between sizes: linear in n; past {hi}, {fmt(slope)} a point more.")
