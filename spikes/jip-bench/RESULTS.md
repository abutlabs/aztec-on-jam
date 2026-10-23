# Native timings of the JIP's host calls, and what a host call can save (#32, #42)

`cargo run --release -- ../kzg-gas/vectors ../ecdsa-gas/vectors/inputs.txt`, 2026-10-23, Apple
M-series Mac, one thread, the median of 25 runs (the machine had been swapping earlier in the
day; treat the numbers as indicative).

| Operation | Native | PVM gas (PolkaJam, the same Rust) | K = PVM gas / native ns |
|---|---|---|---|
| Poseidon2 hash (one permutation), `p2` | 9.5 µs | 428,841 | 44.9 |
| KZG point evaluation, `kzg-verify` (zkcrypto `bls12_381`) | 2,989.8 µs | 199,453,527 | 66.7 |
| secp256k1 recovery, `k256` | 136.9 µs | 13,119,449 | 95.8 |
| BN254 pairing check, 2 pairs, arkworks | 795.9 µs | | |
| BN254 pairing check, 4 pairs, arkworks | 1,161.0 µs | | |
| BN254 G1 MSM, 1, 16, 128 points, arkworks | 92.5, 478.3, 2,115.8 µs | | |
| BLS12-381 pairing check, 2 pairs, blst | 772.1 µs | | |
| BLS12-381 G1 MSM, 1, 16, 128 points, blst | 135.1, 337.0, 422.5 µs | | |

## What a host call can save

The draft compared JAM with Ethereum's precompile prices (about 50×). That overstates it. Ethereum
prices precompiles far below what the same work costs as EVM bytecode, because the EVM
interprets 256-bit arithmetic slowly. JAM's PVM has a recompiler that runs 64-bit arithmetic
close to native, so a host call, priced consistently with ordinary PVM code, saves only:

- **f**, how much slower the recompiled PVM runs the same code than native: measured on x86
  below, 4.7–9.4.
- **times any library gain**: blst's assembly against the pure-Rust code the services run
  (zkcrypto's KZG here takes 2,990 µs, against blst's pairing check at 772 µs).
- **divided by m**, the JIP's safety margin.

So at the draft's m = 2 a host call cuts gas by about 2–4×, not 50×. Poseidon2 drops from
428,841 to 206,400, so a depth-40 insertion (8.3M) fits accumulate's 10M. The JIP's value is
real but narrower: faster on its own terms, constant and deterministic cost, audited libraries
every client shares, and just enough headroom for accumulate to update a tree.

## R and f on x86 (#42)

`.github/workflows/jip-pricing.yml`, run 40020001416, 2026-10-23: GitHub's 4-vCPU runner (AMD
EPYC 7763), PolkaJam v0.8.0 with its recompiler ("PVM backend: compiler"). The workflow runs
the kzg, ecdsa and poseidon2 spikes on a six-validator testnet, this program natively on the
same runner, and `pricing.py` on both:

| Code | PVM gas | Recompiled refine, median | Native | R = gas / ns | f = PVM time / native |
|---|---|---|---|---|---|
| KZG point evaluation | 199,453,527 | 25.277 ms | 4.243 ms | 7.9 | 6.0 |
| secp256k1 recovery | 13,119,449 | 8.374 ms | 0.201 ms | 1.6 | 41.7 |
| secp256k1 recovery × 33 | 432,756,815 | 62.379 ms | 6.634 ms | 6.9 | 9.4 |
| Poseidon2 × 100 | 42,892,428 | 6.130 ms | 1.294 ms | 7.0 | 4.7 |

The single recovery's refine is mostly the package's fixed cost: 33 recoveries take 1.9 ms
each. Leaving it out, **R ≈ 7–8 gas/ns and f ≈ 4.7–9.4**. Earlier estimates from #14's
UltraHonk timings (R 7–21, f 3.5–10) bracketed this. The KZG row's gas is the current build's; until ledger 52 the docs quoted 199,473,075, from an earlier build of the kzg service.

Native times on the same runner, one thread, median of 25:

| Operation | Native |
|---|---|
| Poseidon2 hash, `p2` | 12.9 µs |
| KZG point evaluation, zkcrypto | 4,243.0 µs |
| secp256k1 recovery, k256 | 201.0 µs |
| BN254 pairing check, 2 / 4 pairs, arkworks | 1,279.5 / 1,892.3 µs |
| BN254 G1 MSM, 1 / 16 / 128 points, arkworks | 131.2 / 633.2 / 2,911.1 µs |
| BLS12-381 pairing check, 2 pairs, blst | 974.4 µs |
| BLS12-381 G1 MSM, 1 / 16 / 128 points, blst | 168.0 / 971.8 / 1,490.8 µs |

The JIP's constants (`docs/jip/jip-draft.md`, *Gas pricing*) are these times × R = 8 × m = 2.

K, PVM gas per native nanosecond of the *same* code, is 45–96. It measures how PVM gas maps to
native work, not time on a validator (that is R), and it varies with the instruction mix.

## End to end, as the final JIP specifies the calls (#45)

The simulated reviews (#33, GW-3 and ZW-10, both blocking) found that the timings above leave
out work the calls must do. They take bare kernels on prepared points: no decoding, no on-curve
or subgroup checks, and blst's MSM across several threads (blst parallelizes by default; it is
now built `no-threads`). `src/bin/e2e.rs` times each core call as a client would run it:

- **Input:** the encoded input in the EIPs' encodings (EIP-196/197, EIP-2537), with every
  canonical-field, on-curve and subgroup check, any 32-byte scalar, and k = 0 holding.
- **Cases:** random group elements, and adversarial cases: one base repeated, P with −P, half
  at infinity, scalars r − 1 and 2^256 − 1, a pair repeated, and a G2 point outside the
  subgroup last.
- **Implementations:** two each, which must agree on every input. For BN254, arkworks and
  substrate-bn (revm's; it has no MSM, so a sum of multiplications). For BLS12-381, blst and
  arkworks.
- **Statistics:** 101 single-threaded runs each, giving the median, p99 and maximum.

`e2e-pricing.py` turns the reference implementation's worst-case p99 into gas under the final
JIP's invariant: 10/3 gas/ns (the GP's sizing of 5e9 gas as 1.5 s of a core,
`discussion.tex:77`) times m = 2. The x86 run is `.github/workflows/jip-e2e.yml`, run
40026200133, on GitHub's 4-vCPU runner (AMD EPYC 9V45). Its raw timings are
`ci/e2e-40026200133.txt`. **The implementations agree on all 144 inputs.**

`./e2e-pricing.py < ci/e2e-40026200133.txt`:

#### `bn254_g1_msm` (reference arkworks)

| n | reference p99, worst case | worst case | other library's worst p99 | gas |
|---|---|---|---|---|
| 1 | 105.5 µs | random | 91.8 µs | 703,667 |
| 2 | 131.9 µs | random | 175.1 µs | 879,660 |
| 4 | 170.4 µs | random | 449.6 µs | 1,135,920 |
| 8 | 476.7 µs | random | 1,711.3 µs | 3,178,140 |
| 16 | 445.5 µs | scalars 2^256 − 1 | 3,007.9 µs | 3,178,140 |
| 32 | 961.8 µs | scalars 2^256 − 1 | 3,392.4 µs | 6,411,967 |
| 64 | 1,201.1 µs | random | 7,404.2 µs | 8,007,640 |
| 128 | 1,927.8 µs | random | 16,347.4 µs | 12,852,234 |
| 256 | 4,621.4 µs | scalars r − 1 | 29,243.0 µs | 30,809,314 |
| 512 | 5,920.7 µs | random | 58,980.4 µs | 39,471,280 |

Gas between sizes: linear in n; past 512, 33,836 a point more.

#### `bn254_pairing_check` (reference arkworks)

| n | reference p99, worst case | worst case | other library's worst p99 | gas |
|---|---|---|---|---|
| 1 | 934.1 µs | random, holds | 1,355.7 µs | 6,227,547 |
| 2 | 1,093.2 µs | random, holds | 2,029.6 µs | 7,287,687 |
| 4 | 1,683.5 µs | random, holds | 3,375.7 µs | 11,223,507 |
| 8 | 3,066.3 µs | random, holds | 6,000.8 µs | 20,442,107 |

Gas = 4,196,895 + k × 2,030,652, at or above every measured k.

#### `bls12_381_g1_msm` (reference blst)

| n | reference p99, worst case | worst case | other library's worst p99 | gas |
|---|---|---|---|---|
| 1 | 126.4 µs | scalars r − 1 | 208.1 µs | 842,540 |
| 2 | 179.8 µs | random | 321.6 µs | 1,198,814 |
| 4 | 899.6 µs | scalars 2^256 − 1 | 731.2 µs | 5,997,400 |
| 8 | 557.4 µs | scalars 2^256 − 1 | 810.3 µs | 5,997,400 |
| 16 | 2,188.4 µs | P and −P | 1,473.7 µs | 14,589,460 |
| 32 | 2,451.2 µs | P and −P | 3,141.8 µs | 16,341,307 |
| 64 | 4,516.1 µs | random | 9,467.5 µs | 30,107,254 |
| 128 | 9,917.9 µs | random | 9,708.3 µs | 66,119,300 |
| 256 | 12,407.2 µs | one base repeated | 18,199.0 µs | 82,714,587 |
| 512 | 31,344.3 µs | random | 42,159.0 µs | 208,961,727 |

Gas between sizes: linear in n; past 512, 493,153 a point more.

#### `bls12_381_pairing_check` (reference blst)

| n | reference p99, worst case | worst case | other library's worst p99 | gas |
|---|---|---|---|---|
| 1 | 533.2 µs | random, holds | 930.9 µs | 3,554,574 |
| 2 | 762.5 µs | one pair repeated | 1,256.0 µs | 5,083,487 |
| 4 | 1,274.9 µs | one pair repeated | 2,030.1 µs | 8,499,314 |
| 8 | 2,346.8 µs | one pair repeated | 3,486.9 µs | 15,645,254 |

Gas = 1,827,334 + k × 1,727,240, at or above every measured k.

**Against the first draft's constants**, which were bare kernels at R = 8:
- **BLS12-381 MSM:** 128 points cost 66.1M, against about 23.9M before. It was underpriced
  about 2.8×, because of blst's threads and the per-point subgroup checks EIP-2537 requires.
- **BN254 pairing:** 2 pairs cost 8.26M, against 20.47M. BN254 MSMs are about 3–3.6× cheaper
  than before. The checks cost less than the change of anchor saves.

**Caveats.** These are one run's p99 on a shared VM, and noise shows: the BLS12-381 MSM's
4-point p99 exceeds its 8-point one. Each table entry is raised to the largest at any smaller
size, so gas never falls as n grows. Before submission, the constants need runs on
pinned-frequency bare metal, with more than one machine.

`e2e --vectors` writes `docs/jip/vectors/jip8-core.jsonl`, 74 JAM-native vectors from both
implementations. They cover the cases above at n ≤ 4, plus malformed input: a wrong length,
x equal to the modulus, a point off the curve, nonzero padding in an EIP-2537 field, and a
BLS12-381 G1 point outside the subgroup.
