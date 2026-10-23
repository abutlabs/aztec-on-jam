# EIP-4844's KZG checks in refine, on PolkaJam (#24)

`spikes/kzg-gas/measure.sh`, 2026-10-23, Apple M-series Mac, polkajam-testnet v0.8.0 (six
validators, dummy finality, the PVM interpreter), via `spikes/lib/polkajam.sh`.

**Gas is the current code's,** from CI run 40020001416 (the jip-pricing workflow, x86-64,
PolkaJam's recompiler), which ran this script from clean. Gas does not depend on the machine or
the PVM backend. The refine times are the Mac interpreter's, from an earlier build of the
service whose gas differed by at most 0.1% (ledger 52).

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
| point evaluation: geth pointEvaluation1 | 199,453,527 | 19.9% | 4.0% | 0.193 s | 01 |
| point evaluation: c-kzg correct_proof_1_2 | 199,453,527 | 19.9% | 4.0% | 0.193 s | 01 |
| point evaluation: c-kzg correct_proof_4_4 | 199,453,527 | 19.9% | 4.0% | 0.219 s | 01 |
| point evaluation: c-kzg correct_proof_6_5 | 199,453,527 | 19.9% | 4.0% | 0.343 s | 01 |
| point evaluation: geth's, y altered | 199,453,527 | 19.9% | 4.0% | 0.197 s | 00 |
| point evaluation: c-kzg incorrect_proof_2_3 | 199,453,527 | 19.9% | 4.0% | 0.190 s | 00 |
| blob (131,072 B) against its commitment: correct_proof, first | 262,626,180 | 26.3% | 5.3% | 0.256 s | 01 |
| blob: correct_proof, fourth | 262,626,180 | 26.3% | 5.3% | 0.262 s | 01 |
| blob: incorrect_proof | 262,626,175 | 26.3% | 5.3% | 0.257 s | 00 |

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

**Blobs as extrinsics, exported as segments.** The same check with the blob carried as the
work-item's extrinsic (`jamboree rpc submit --extrinsic`), and optionally exported:

| case | package | extrinsics | refine gas | of a tiny package | PolkaJam refine | verdict |
|---|---|---|---|---|---|---|
| 1 blob | 401 B | 131,072 B | 262,157,756 | 26.2% | 0.251 s | 01 |
| 1 blob, exported (32 segments) | 401 B | 131,072 B | 262,275,221 | 26.2% | 0.250 s | 01 |
| 3 blobs, exported (96 segments) | 666 B | 393,216 B | 786,812,809 | 78.7% | 0.727 s | 01 |
| 4 blobs | 798 B | 524,288 B | 999,999,440, out of gas | 100% | 0.950 s | none |

**Batch-checked** (c-kzg's `verify_blob_kzg_proof_batch`, all 24 of its vectors agreeing on the
host): one pairing over a random linear combination, the blobs as extrinsics.

| blobs | refine gas | of a tiny package | PolkaJam refine | verdict |
|---|---|---|---|---|
| 2 | 404,223,089 | 40.4% | 0.408 s | 01 |
| 3 | 545,169,253 | 54.5% | 0.525 s | 01 |
| 3, one wrong proof | 545,169,253 | 54.5% | 0.525 s | 00 |
| 6 (an Aztec checkpoint) | 967,993,490 | 96.8% | 0.934 s | 01 |

From the 2- and 3-blob runs: 122,330,761 fixed (the pairing and the challenge) plus
140,946,164 a blob (hashing, evaluating, three G1 multiplications), which predicts
968,007,745 for six. Six measured 967,993,490, 0.0015% off. So batched, **a checkpoint's 6
blobs fit one tiny package**, 34 fit a full one, and an epoch's 192 cost about 27.2B gas
(5.4 full packages), against 50.4B (10.1) checked one by one. The G1 multiplications are a
naive sum; a multi-scalar multiplication would cut the per-blob part further.

**Imported by a later package** (`jamboree rpc submit --import`), the exported blob's 32
segments re-checked:

| case | refine gas | PolkaJam refine | verdict |
|---|---|---|---|
| imported by segments root (from the exporter's report, `jamboree rpc report`) | 263,028,551 | 0.251 s, 3 guarantors | 01 |
| the same, a wrong proof | 263,028,551 | 0.250 s, 3 guarantors | 00 |
| imported by work-package hash (the tagged form), 11 slots after the export | 263,028,551 | 0.244 s, the primary only | not reported (#38, below) |

**Why the tagged form failed (#38).** `tagged-import.sh` imports the same 32 segments by
package hash right after the export (package 21), by package hash again 11 slots later (22), and
by segments root after the same wait (23):

| package | imports | anchor slot (exporter's 9509450) | refines | outcome |
|---|---|---|---|---|
| 21 | by package hash, at once | 9509451 | 3 | reported at 9509453, verdict 01 |
| 22 | by package hash, after 11 slots | 9509463 | 1 | "Not reported in time" |
| 23 | by segments root, after 11 slots | 9509470 | 3 | reported at 9509472, verdict 01 |

For 22, both secondaries log `WorkPackageFailed { reason: "Segment-root lookup is invalid" }`
(JIP-3) and drop the primary's share. The primary then fails: "Not enough signatures from
secondary guarantors". This is the GP's rule, not a fault: a report's segment-root lookup may
name only packages in the recent history (`reporting_assurance.tex:401-423`), the last H = 8
blocks (`definitions.tex:269`). The primary alone resolves the older mapping, from its own
store. **So a builder imports by package hash only within 8 blocks of the export, and by
segments root after that.** Segments stay in the DA for 28 days either way.

Importing 32 segments adds about 0.87M gas over the same check from an extrinsic (27,000 a
segment). By segments root, the round trip works: a blob checked and exported by one
package, kept in the DA, is read back and re-checked by another.

Exporting 32 segments adds 117,465 gas (about 3,700 a segment): binding a blob costs the
check, not the export. Per package, gas is the limit, not size: three blobs fit a tiny
package's 1e9 and four run out, so a full package (5e9) holds 19.

**One blob per package, as a payload.** PolkaJam refuses payloads above 204,533 bytes (#36).
That is JAM-NP's 200 KiB limit on the work-package message, which holds payloads, so one
blob fits and two do not. Extrinsics avoid the limit: they travel separately, bounded by the
Gray Paper's 13,791,360-byte bundle.

**Still open (#24):** where blob data lives on JAM, and for how long; the batch check.
