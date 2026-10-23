# Native timings of the JIP's host calls, and what a host call can save (#32, #42)

`cargo run --release -- ../kzg-gas/vectors ../ecdsa-gas/vectors/inputs.txt`, 2026-10-23, Apple
M-series Mac, one thread, the median of 25 runs (the machine had been swapping earlier in the
day; treat the numbers as indicative).

| Operation | Native | PVM gas (PolkaJam, the same Rust) | K = PVM gas / native ns |
|---|---|---|---|
| Poseidon2 hash (one permutation), `p2` | 9.5 µs | 428,839 | 44.9 |
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
428,839 to 206,400, so a depth-40 insertion (8.3M) fits accumulate's 10M. The JIP's value is
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
UltraHonk timings (R 7–21, f 3.5–10) bracketed this. The KZG row's gas differs from the local
199,453,527 by the median vector's input.

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
