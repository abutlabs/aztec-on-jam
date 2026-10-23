# Native timings of the JIP's host calls, and what a host call can save (#32, #42)

`cargo run --release -- ../kzg-gas/vectors ../ecdsa-gas/vectors/inputs.txt`, 2026-10-23, Apple
M-series Mac, one thread, the median of 25 runs (the machine had been swapping earlier in the
day; treat the numbers as indicative).

| Operation | Native | PVM gas (PolkaJam, the same Rust) | K = PVM gas / native ns |
|---|---|---|---|
| Poseidon2 hash (one permutation), `p2` | 9.5 µs | 428,839 | 44.9 |
| KZG point evaluation, `kzg-verify` (zkcrypto `bls12_381`) | 2,989.8 µs | 199,473,075 | 66.7 |
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

- **f**, how much slower the recompiled PVM runs the same code than native. From CI's x86
  runner: 169M gas of UltraHonk in 8–24 ms in 15 of 30 refines (#14), against about 2.3 ms
  natively, so f ≈ 3.5–10 and R (gas per nanosecond of recompiled execution) ≈ 7–21. The other
  12 refines took 0.41–0.56 s, unexplained. Measured properly in #42.
- **times any library gain**: blst's assembly against the pure-Rust code the services run
  (zkcrypto's KZG here takes 2,990 µs, against blst's pairing check at 772 µs).

So a host call would cut gas by roughly 2–10×, depending on the operation, not 50×. For example,
at R = 21 and m = 1:
- **Poseidon2:** 9.5 µs × 21 ≈ 200,000 gas, against 428,839 as PVM code (about 2×). A
  depth-40 insertion would be about 8M gas, just inside accumulate's 10M.
- **KZG point evaluation:** with blst, about 1 ms × 21 ≈ 21M gas, against 199M (about 9×).

The JIP's value is real but narrower: faster on its own terms, constant and deterministic cost,
audited libraries every client shares, and just enough headroom for accumulate to update a
tree. Its constants wait on #42.

K, PVM gas per native nanosecond of the *same* code, is 45–96. It measures how PVM gas maps to
native work, not time on a validator (that is R), and it varies with the instruction mix.
