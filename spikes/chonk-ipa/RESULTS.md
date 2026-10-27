# A CHONK proof's verification, split: the reduction and the deferred IPA (#46)

`spikes/chonk-ipa/measure.sh`, 2026-10-27, on this Mac (Apple M-series, 10 cores), every
measurement on one thread. Barretenberg 6.0.0, the version Aztec v6 proves with: its released `bb`
for the whole check, and the same version built from source (`MULTITHREADING=OFF`) for the IPA.

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

**The reduction** is the whole check minus one discharge: about **71 ms** (42% of a single
proof's verification; the IPA is the other 58%).

| Batch | Native time a proof | Saved against one proof alone |
| --- | --- | --- |
| 1 | about 170 ms | |
| 8 | about 85 ms (71 + 108/8) | 50% |
| 64 | about 74 ms (71 + 179/64) | 57% |
| 512 | about 72.5 ms (71 + 746/512) | 57% |

## In refine gas: an estimate, not a measurement

No PVM CHONK verifier exists. The estimate scales by `spikes/chonk`'s ratio: our UltraHonk proof
verifies in 2.2 to 2.4 ms natively and in 169,112,761 gas in refine, without host calls. That is
70 to 77M gas a native millisecond.

| Batch | Gas a proof, by the ratio | Full packages (5e9) a proof |
| --- | --- | --- |
| 1 | 11.8 to 13.4B | 2.4 to 2.7 |
| 8 | 6.0 to 6.5B | 1.2 to 1.3 |
| 64 | 5.2 to 5.7B | 1.0 to 1.1 |
| 512 | 5.1 to 5.6B | 1.0 to 1.1 |

**Batching halves the cost, and no more.** The deferred IPA, 58% of one proof's verification,
nearly vanishes in a batch. But the reduction stays at about 71 ms a proof, about 5.2B gas by the
ratio. So even amortised, one proof needs about a full package without host calls.

The ratio is crude: the reduction's work is not the UltraHonk verifier's. It is several sumchecks
and Merge, Translator and MegaZK commitments, with KZG pairings and BN254 MSMs. Which share of its
71 ms the proposed curve calls (BN254 MSM and pairing) would take, and which share is field
arithmetic and Poseidon2 hashing, is #62's first step: a profile of bb's verifier by component.
That share, not the ratio, decides whether a direct check of Aztec's client proofs fits a package.
