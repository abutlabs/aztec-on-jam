# EIP-4844's point evaluation in refine, on PolkaJam (#24)

`spikes/kzg-gas/measure.sh`, 2026-10-23, Apple M-series Mac, polkajam-testnet v0.8.0 (six
validators, dummy finality, the PVM interpreter), via `spikes/lib/polkajam.sh`.

**What it checks.** Aztec's epoch proof commits to the epoch's transaction data as EIP-4844
blobs, batched into one KZG opening (`FinalBlobAccumulator {blob_commitments_hash, z, y, C}`).
Ethereum's Rollup contract checks it once per epoch proof with the 0x0a point-evaluation
precompile (`BlobLib.sol`, `EpochProofLib.sol`). On JAM that check is PVM code in refine.

**The code.** `kzg/` is c-kzg-4844's `verify_kzg_proof` in no_std Rust on zkcrypto's
`bls12_381` 0.8: e(pi, [s]G2 - [z]G2) == e(C - [y]G1, G2), as one multi-Miller loop and one
final exponentiation, with [s]G2 from Ethereum's ceremony (c-kzg `trusted_setup.txt`).
`point_evaluation` adds the precompile's versioned-hash check (0x01 || sha256(C)[1..]).

**Checked against Ethereum first.** On the host, all 122 of c-kzg-4844's `verify_kzg_proof`
vectors agree (54 valid, 48 invalid, 20 malformed and refused), and geth's precompile vector
passes (and fails with y altered).

| case | refine gas | of a tiny package (1e9) | of a full one (5e9) | PolkaJam refine | accumulate's verdict |
|---|---|---|---|---|---|
| geth pointEvaluation1 | 199,469,377 | 19.9% | 4.0% | 0.190 s (3 guarantors) | 01 |
| c-kzg correct_proof_1_2 | 199,469,377 | 19.9% | 4.0% | 0.194 s | 01 |
| c-kzg correct_proof_4_4 | 199,469,377 | 19.9% | 4.0% | 0.210 s | 01 |
| c-kzg correct_proof_6_5 | 199,469,377 | 19.9% | 4.0% | 0.199 s | 01 |
| geth's, y altered | 199,469,377 | 19.9% | 4.0% | 0.193 s | 00 |
| c-kzg incorrect_proof_2_3 | 199,469,377 | 19.9% | 4.0% | 0.192 s | 00 |

The gas is the same in every case: the crate is constant-time.

**Beside Ethereum, like with like.** Gas units differ between the two chains, so compare shares
of a block's budget, not numbers. The precompile charges a fixed 50,000 gas (EIP-4844's
`POINT_EVALUATION_PRECOMPILE_GAS`), about 0.1% of a block. On JAM the same check takes 4.0% of
a full package's refine budget, or 20% of a tiny one. Ethereum prices this check as a native
operation; on JAM it is interpreted PVM code (#31).

**With the epoch proof.** The epoch proof (about 169M at 2^24, #23) plus this check comes to
about 369M gas: 7.4% of a full package, or 37% of a tiny one, once per epoch.

**Not yet measured (#24):** binding each blob's data to its commitment, the job Ethereum's
consensus layer does for the contract. On JAM nobody does it for free; it costs
`verify_blob_kzg_proof` per blob (a barycentric evaluation over 4,096 field elements, plus
the same pairing check). Also open: where the blob data lives on JAM.
