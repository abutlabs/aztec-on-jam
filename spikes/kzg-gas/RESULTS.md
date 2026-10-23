# EIP-4844's KZG checks in refine, on PolkaJam (#24)

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
vectors and all 29 of its `verify_blob_kzg_proof` vectors agree (valid accepted, invalid
rejected, malformed refused), and geth's precompile vector passes (and fails with y altered).
Vectors from pinned commits (c-kzg a751e2d, geth 2006f08).

| case | refine gas | of a tiny package (1e9) | of a full one (5e9) | PolkaJam refine (3 guarantors) | accumulate's verdict |
|---|---|---|---|---|---|
| point evaluation: geth pointEvaluation1 | 199,473,075 | 19.9% | 4.0% | 0.193 s | 01 |
| point evaluation: c-kzg correct_proof_1_2 | 199,473,075 | 19.9% | 4.0% | 0.193 s | 01 |
| point evaluation: c-kzg correct_proof_4_4 | 199,473,075 | 19.9% | 4.0% | 0.219 s | 01 |
| point evaluation: c-kzg correct_proof_6_5 | 199,473,075 | 19.9% | 4.0% | 0.343 s | 01 |
| point evaluation: geth's, y altered | 199,473,075 | 19.9% | 4.0% | 0.197 s | 00 |
| point evaluation: c-kzg incorrect_proof_2_3 | 199,473,075 | 19.9% | 4.0% | 0.190 s | 00 |
| blob (131,072 B) against its commitment: correct_proof, first | 262,430,040 | 26.2% | 5.2% | 0.256 s | 01 |
| blob: correct_proof, fourth | 262,430,040 | 26.2% | 5.2% | 0.262 s | 01 |
| blob: incorrect_proof | 262,430,001 | 26.2% | 5.2% | 0.257 s | 00 |

The gas barely varies: the crate is constant-time. A blob check is the pairing check plus about
63M gas for hashing the blob (the Fiat-Shamir challenge) and evaluating its polynomial over
4,096 field elements (one batched inversion).

**Beside Ethereum, like with like.** Gas units differ between the two chains, so compare shares
of a block's budget, not numbers. The precompile charges a fixed 50,000 gas (EIP-4844's
`POINT_EVALUATION_PRECOMPILE_GAS`), about 0.1% of a block. On JAM the same check takes 4.0% of
a full package's refine budget, or 20% of a tiny one. Ethereum prices this check as a native
operation; on JAM it is interpreted PVM code (#31).

**With the epoch proof.** The epoch proof (about 169M at 2^24, #23) plus this check comes to
about 369M gas: 7.4% of a full package, or 37% of a tiny one, once per epoch.

**Per blob, on JAM.** On Ethereum, consensus checks each blob against its commitment before
any contract sees it. On JAM a service would do that itself, at 262M gas a blob. An Aztec
epoch can carry 32 checkpoints of up to 6 blobs: 192 blobs, about 50 billion gas if each is
checked alone, or ten full packages per epoch. That makes data binding, not the epoch proof,
the expensive part. Ways down, to measure: c-kzg's batch verification (one pairing for many
blobs; the 63M per blob remains), checking only the blobs an epoch actually uses, or letting
JAM's own availability (erasure-coded package data, bound by hash) stand in for the KZG
binding where Aztec's circuits allow it.

**One blob per package, in practice.** PolkaJam refuses packages over about 256 KiB (#36),
against the Gray Paper's 13,791,360-byte bundle limit: one blob fits, two do not.

**Still open (#24):** where blob data lives on JAM, and for how long; the batch check.
