#!/usr/bin/env python3
"""#42: R (PVM gas per nanosecond of execution, as PolkaJam ran it) and f (that time over the same
Rust code's native time), on one machine. Reads the three spikes' PolkaJam telemetry
(out/refined-<tag>.log) and jip-bench's "NATIVE <name> <ns>" lines (stdin).
  pricing.py <spikes dir> <backend> < native.txt"""
import re, statistics as st, sys
spikes, backend = sys.argv[1], sys.argv[2]
native = {l.split()[1]: float(l.split()[2]) for l in sys.stdin if l.startswith("NATIVE ")}

def refines(spike, tag):
    rows = re.findall(r"RefineCost \{ total: ExecCost \{ gas: (\d+), ns: (\d+) \}", open(f"{spikes}/{spike}/out/refined-{tag}.log").read())
    return [(int(g), int(ns)) for g, ns in rows]

# (operation, spike, tag, operations in that package, native key)
cases = [("KZG point evaluation", "kzg-gas", "1", 1, "kzg_point_evaluation"),
         ("secp256k1 recovery", "ecdsa-gas", "1", 1, "secp256k1_recover"),
         ("secp256k1 recovery x 33", "ecdsa-gas", "2", 33, "secp256k1_recover"),
         ("Poseidon2 hash x 100", "poseidon2-gas", "3", 100, "poseidon2_hash_(one_permutation)")]
print(f"| operation | PVM gas | PolkaJam refine ({backend}), median | native | R = gas / ns | f = PVM time / native |")
print("|---|---|---|---|---|---|")
for name, spike, tag, n, key in cases:
    r = refines(spike, tag)
    gas = min(g for g, _ in r)
    ns = st.median(t for _, t in r)
    nat = native[key] * n
    print(f"| {name} | {gas:,} | {ns / 1e6:.3f} ms | {nat / 1e6:.3f} ms | {gas / ns:.1f} | {ns / nat:.1f} |")
