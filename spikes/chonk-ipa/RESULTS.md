# A CHONK proof's verification, split: the reduction and the deferred IPA (#46)

`spikes/chonk-ipa/measure.sh`, 2026-10-27, on this Mac (Apple M-series, 10 cores), every
measurement on one thread. Barretenberg 6.0.0, the version Aztec v6 proves with: its released `bb`
for the whole check, the same version built from source (`MULTITHREADING=OFF`) for the IPA, and a
second source build (threading on, run on one thread) with bb's timing scopes extended to verify.

## How bb verifies a CHONK proof

The verbose verifier (`bb verify --scheme chonk -v`) checks the proof in stages:
1. MegaZK (Oink and sumcheck);
2. the databus;
3. Merge;
4. the ECCVM sumcheck;
5. the Translator.

The ECCVM part ends in a TripleIPA claim over Grumpkin at 2^15 (`CONST_ECCVM_LOG_N`). That claim
is folded into a deferred accumulator, 15 round challenges and a claimed point G_0
(`IPA::NativeAccumulator`). It is discharged by one 2^15-point MSM against the Grumpkin SRS,
`IPA::batch_verify_accumulators`. That function takes any number of accumulators, so a batch of
proofs pays the MSM once (`ChonkBatchVerifier`). Everything before the discharge is the
per-proof **reduction**.

## Measured

| What | Native time, one thread |
| --- | --- |
| A whole CHONK verification (`bb verify`, the pinned sponsored-fee transfer, startup subtracted, three runs of 20) | 169 to 176 ms |
| One IPA verify at 2^15 (`ipa_verify`, patched to load its proof once) | 99.3 to 100.0 ms |
| Discharge of 1 accumulator | 99.0 ms |
| Discharge of 8 | 108 ms |
| Discharge of 64 | 178 to 181 ms |
| Discharge of 512 | 740 to 753 ms |

The discharge is one MSM of about 98 ms, plus about 1.27 ms a proof: each accumulator's
challenge polynomial (2^15 field elements) is built and added in, and its claimed point scaled.

## Where the rest goes: setup, and the per-proof reduction

The whole check minus one discharge is about 71 ms, but most of that is not per-proof work. bb's
own timing scopes, extended to `verify` and to each step (`bb_verify_bench.patch`, Barretenberg
6.0.0 from source, one thread, five runs; a run's tree is in `verify-split.txt`), show:

| Part of one `bb verify` | Time | Kind |
| --- | --- | --- |
| Grumpkin CRS for the ECCVM verifier (`VerifierCommitmentKey(1)`) | 23.4 to 25.1 ms | setup: loading the CRS file, once a process |
| Grumpkin CRS and key for the discharge (32,768 points) | about 26 ms | setup, once a process |
| MegaZK Oink | 4.8 to 5.3 ms | per proof; 4.35 ms of it Poseidon2 transcript hashing |
| ECCVM reduction | 6.2 to 7.4 ms | per proof; 2.0 ms of it Poseidon2 |
| Translator and the joint sumcheck and PCS | 3.9 to 4.4 ms | per proof; 1.76 ms Poseidon2, 1.75 ms a BN254 KZG batch-mul |
| Merge | 0.7 to 1.0 ms | per proof |
| Four BN254 pairing checks (kernel, Merge, joint) | about 2.5 ms | per proof |
| TripleIPA folded into the accumulator | 1.2 ms | per proof |
| The discharge's 2^15 Grumpkin MSM | about 106 ms | once a batch |

**The per-proof work is about 19 ms**, and is made of:
- Poseidon2 hashing of the transcript, about 8.1 ms (43%);
- BN254 curve work, about 4.5 ms (25%): the KZG batch-mul, the pairings and Merge's commitments;
- the rest, about 6.4 ms (32%): sumcheck and other field arithmetic, and the accumulator.

**The setup**, about 50 ms, is loading the Grumpkin CRS from bb's files. A verifier pays it once,
not per proof. In a JAM service the CRS (2^15 points, about 2 MB) would be constant data in the
service's preimages.

| Batch | Native time a proof (19 + discharge / N) |
| --- | --- |
| 1 | about 118 ms |
| 8 | about 32.5 ms (19 + 108/8) |
| 64 | about 22 ms (19 + 179/64) |
| 512 | about 20.5 ms (19 + 746/512) |

## In refine gas: an estimate, not a measurement

No PVM CHONK verifier exists. The estimate scales by `spikes/chonk`'s ratio: our UltraHonk proof
verifies in 2.2 to 2.4 ms natively and in 169,112,761 gas in refine, without host calls. That is
70 to 77M gas a native millisecond.

| Batch | Gas a proof, by the ratio | Proofs a full package (5e9) |
| --- | --- | --- |
| 1 | 8.3 to 9.1B | 0.55 to 0.6 |
| 8 | 2.3 to 2.5B | 2 |
| 64 | 1.5 to 1.7B | about 3 |
| 512 | 1.4 to 1.6B | about 3 |

**Batching cuts the cost about sixfold.** The 2^15 Grumpkin MSM, shared, nearly vanishes from a
large batch. What remains is the 19 ms reduction, about 1.4 to 1.6B gas by the ratio: about three
of Aztec's client proofs a full package, without host calls.

**With the proposed curve calls**, the BN254 work (4.5 ms) and the shared Grumpkin MSM would run
natively. About 15 to 16 ms a proof would stay in the PVM: Poseidon2 8.1 ms, field arithmetic and
the accumulator 6.4 ms, the discharge's 1.27 ms a proof. That is about 1.1 to 1.2B gas by the
ratio, plus the calls' own prices. Poseidon2 is then the largest part of what is left. That is a
measured reason for a Poseidon2 call (#62).

The ratio is crude: PVM cost differs by kind of work (field multiplication, hashing, curve
arithmetic). These numbers rank the parts. They do not price them.

## Correction (2026-10-27)

The first version of this page (2239829) called the whole check minus one discharge, 71 ms, the
per-proof reduction, and estimated 5.2 to 5.7B gas a batched proof. It counted the ~50 ms CRS
setup as per-proof work. The per-proof reduction is about 19 ms.
