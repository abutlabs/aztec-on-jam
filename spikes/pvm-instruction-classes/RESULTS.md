# The curve kernels by PVM instruction class (#47)

`spikes/pvm-instruction-classes/measure.sh`, 2026-10-27, lasair main at d75d6db (GP 0.8.0 gas model),
the curve calls off. Each kernel is the PVM code that `spikes/jip8-lasair` measured, with the
same blobs and payloads. The profiled block gas matches the measured refine gas to within 0.03%;
the rest is host-call gas.

Gas is the protocol's, the same on every conforming client. The split by instruction is an
approximation, because the GP charges a basic block, not its instructions: each block's gas is
spread evenly over its instructions.

| Kernel | Gas | Multiply | Add, sub, carry | Load, store | Widening multiply saves | Add-with-carry saves |
|---|---|---|---|---|---|---|
| pvm-bn254_pairing_check-2 | 105.1M | 9.2% | 32.5% | 45.9% | 4.6% | 8.2% |
| pvm-bn254_g1_msm-128 | 476.3M | 11.5% | 35.3% | 38.2% | 5.6% | 8.8% |
| pvm-bls12_381_pairing_check-2 | 194.7M | 7.9% | 35.0% | 38.8% | 3.2% | 6.8% |
| pvm-bls12_381_g1_msm-128 | 1465.4M | 12.1% | 37.1% | 39.0% | 6.0% | 9.8% |
| ultrahonk-2^24-field | 155.9M | 9.2% | 32.8% | 42.1% | 4.6% | 7.7% |
| kzg-point-evaluation | 199.5M | 9.2% | 39.4% | 40.2% | 3.9% | 9.3% |

**Multiplication is not where the gas goes.**
- Multiply instructions are 8 to 12% of the gas.
- Additions, subtractions and carry detection are 33 to 39%.
- Loads and stores of limbs are 38 to 46%. Field elements live in memory between operations.

**A widening multiply** (one instruction for a 64x64->128 product, in place of `mul_64` and
`mul_upper_u_u`) would save 3 to 6% of the gas. **An add-with-carry** (in place of the
`set_lt_u` that detects a limb's carry) would save 7 to 10%. Together, about 11 to 16%.

The proposed curve calls save 7 to 25 times the gas (`spikes/jip8-lasair`). An instruction-level
change helps every field, including those the calls leave out, but on the curves it recovers
an eighth to a sixth of the cost. The rest is the PVM's per-instruction cost, and the memory
traffic of 254- and 381-bit values handled 64 bits at a time.
