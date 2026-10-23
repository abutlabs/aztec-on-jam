# Review of JIP-8 (draft at 4f0fd19): Cryptographic host calls

*Simulated review: written by an AI in the persona of Zac Williamson for internal QA. It is not Zac Williamson's view and has not been seen by him.*

## Overall assessment

The draft picks the right level. It proposes whole curve operations (MSM, multi-pairing) and one hash permutation. It does not propose a "verify UltraHonk" call or field-arithmetic calls, and it is candid that the gain is 2–4×, not the 50× that Ethereum's precompile prices suggest. Three things stop it going out. First, the benchmark behind the constants does not time what the spec makes a call do: decoding, on-curve and subgroup checks are left out, and the BLS12-381 MSM is timed on n copies of the generator. Second, the draft never estimates the gain for the case that motivates it, an UltraHonk verification, even though its own measurements are enough to do so. Third, open question 3 (Grumpkin) can be answered from Barretenberg's code today: Aztec's settlement path does not need Grumpkin, and checking CHONK directly does. I also have specific concerns about the KZG call's price, the shape of the MSM pricing, and how far Poseidon2 actually helps accumulate.

Paths below are relative to `aztec-packages/` (v6.0.0) unless they start with `spikes/` or `docs/`, which are in this repository.

## Comments

### ZW-1

**Section:** Motivation; Gas pricing, "What this means for the gain"

**Severity:** major

**Comment:** UltraHonk verification is the headline use, yet the draft gives no number for what it costs after the JIP ("The pairing checks and MSMs have no PVM baseline here"). The draft's own data already splits the cost:

- The verifier runs sumcheck before Shplemini and returns early if sumcheck fails (`ultrahonk_verifier/src/lib.rs:127-137`). An altered public input therefore stops at sumcheck. That costs 9,831,418 gas at 2^24 (`spikes/epoch-proof/RESULTS.md`), against 180,791,299 for an accepted proof. So the VK hash, the Keccak transcript, the public-input delta and the whole of sumcheck make up about 5% of the verifier. The other ~95% is Shplemini plus the MSM plus the pairing.
- The MSM has `number_of_unshifted + log_n + 2` points (`lib.rs:330-336`): 62 at LOG_N = 24, plus the aggregation of recursion pairing points (`lib.rs:577-599`). Aztec's own `HonkVerifier.sol` agrees: 62 `ecMul` calls, one 2-pair pairing (`staticcall(gas(), 8, 0x00, 0x180, ...)`, line 5706) and a single `modexp` for its batched inversion (line 2862).
- At the draft's prices, a 64-point BN254 MSM costs about 25.7M (interpolated) and the 2-pair pairing costs 20,472,000. Add ≤9.8M for transcript and sumcheck plus the Shplemini scalar arithmetic (unmeasured, but a few dozen inversions batched into one plus a few hundred field operations). That puts an UltraHonk check at roughly **55–65M gas, about 3×**, so a full package checks about 80 proofs, not 29.

The draft is right to aim the calls at the MSM and the pairing, and right not to propose field-arithmetic calls: sumcheck is not where the time goes.

**Suggested change:** Measure the post-JIP verifier directly. The reference verifier already sends every curve operation through zkVerify's `CurveHooks` trait (`ark-bn254-ext` v0.6.0: `bn254_msm_g1`, `bn254_multi_miller_loop`, `bn254_final_exponentiation`; used at `lib.rs:570` and `:599`). Build it with a hook that costs no PVM gas, run it on PolkaJam, and add the draft's G for each call. Put the result in a "Gain for an UltraHonk check" row of the Motivation table, with the post-sumcheck/pre-sumcheck split above as its justification.

### ZW-2

**Section:** Motivation, "settling the Aztec network on JAM needs about 812M gas an epoch proof and 981M a checkpoint"

**Severity:** major

**Comment:** The draft cites the two Aztec settlement costs but does not say what the JIP does to them. That is the number a reader from Aztec would look for. Here is my estimate from the draft's own constants. The **epoch proof** falls from about 813M (180.8M + 199.5M + 432.8M) to about 60M for the proof (ZW-1), plus 67.9M for KZG as priced (about 20M if composed, ZW-5), plus 33 × 3,216,000 = 106.1M for the attestations: **~185–235M, roughly 3.5–4.4×**. The **checkpoint** gains much less, because most of it is untouched. In the batched check, a blob costs 140.9M (`spikes/kzg-gas/RESULTS.md`): a Fiat–Shamir hash of 131,072 bytes, a barycentric evaluation over 4,096 BLS12-381 scalar-field elements, and three naive G1 multiplications. Only the multiplications and the shared pairing map onto calls 103–104. The hash and evaluation part, about 63M a blob (262.6M − 199.5M for a single blob), stays as PVM code, so six blobs keep about 380M. The checkpoint drops from about 981M to about 400–420M and becomes the larger of the two settlement costs. With 32 checkpoints an epoch, it is also the larger in total by a wide margin.

**Suggested change:** Add a "with JIP-8" column for these two rows, labelled as estimates until they are measured. Then decide openly about the blob path. One option is to add a whole-operation call matching c-kzg's `verify_blob_kzg_proof_batch`: the repository already passes all 24 of its vectors, and it is exactly the check Ethereum's consensus performs. The other is to state that the blob evaluation stays in the PVM, and why (`docs/aztec/BLOBS.md` already records a hash-committed alternative).

### ZW-3

**Section:** §1 `bn254_g1_msm` and §3 `bls12_381_g1_msm` (gas usage); Gas pricing, MSM paragraph

**Severity:** major

**Comment:** There are three problems.

1. **The spec and the pricing disagree.** The specification says gas is `G_BN_MSM_BASE + n · G_BN_MSM_ITEM` (lines 71, 101), which is linear. The pricing section instead interpolates piecewise between 1, 16 and 128 points (lines 210-213). An implementer cannot tell which one is normative. Interpolation is the right idea for small MSMs, and it is good that the draft noticed a single slope overcharges a 1-point call.
2. **Large MSMs are overcharged.** The cost per point keeps falling as n grows (BN254: 33.5 µs a point from 1 to 16, 20.3 µs from 16 to 128), because Pippenger's cost is about n / log n. Extrapolating the 16→128 segment linearly past 128 will overcharge the MSMs that matter most beyond a single verifier: a batch verifier's combined MSM, and a 2^15-point Grumpkin IPA MSM if ZW-12 is adopted.
3. **The 1-point price may exceed the PVM's own cost.** The 1-point price is 131.2 µs × 16 = 2,099,200 gas. From 2^23 to 2^24, an accepted proof costs 1,606,416 more while a rejected one costs only 199,998 more. That puts the PVM's cost for one extra MSM point plus one extra Shplemini round at about 1.4M or less. A verifier that does its scalar multiplications one at a time would pay more with the call than without it. arkworks' `msm` on a single point is not a good timing for a scalar multiplication.

**Suggested change:** Measure n = 1, 2, 4, …, 2^16. Write the normative formula into §1 and §3 as EIP-2537 does: a per-point cost × discount(n) table, with a lookup table in the JIP. Then check that G(1) is below a measured PVM scalar multiplication on the same curve.

### ZW-4

**Section:** §2 `bn254_pairing_check`

**Severity:** minor

**Comment:** Every Barretenberg KZG check pairs against the same two G2 points: [1] and [x] from the BN254 SRS. bb exploits this with precomputed Miller lines: `commitment_schemes/verification_key.hpp:61-62` calls `reduced_ate_pairing_batch_precomputed(..., srs->get_precomputed_g2_lines(), 2)`, and `ecc/curves/bn254/pairing.hpp:28` holds 87 precomputed line coefficients. The generic call makes each verifier send both G2 points (256 bytes), pay a G2 subgroup check on each one every call, and pay for full Miller loops. The KZG call (§5) already builds in Ethereum's [s]G2, so a built-in fixed point is not a new idea for this JIP.

**Suggested change:** Consider a fixed-G2 variant, `bn254_kzg_pairing_check(P0, P1)` (128 bytes in), that checks e(P0, [1]G2) · e(P1, [x]G2) = 1 against the BN254 SRS's G2 points named in the spec, with precomputed lines. Price it from its own benchmark. Keep the generic call for everything else.

### ZW-5

**Section:** §5 `kzg_point_evaluation`; Gas pricing table

**Severity:** major

**Comment:** `G_KZG` = 67,888,000 is priced from zkcrypto's pure-Rust `bls12_381` (4,243 µs), while *Reference implementations* lists c-kzg-4844, which uses blst. Under the draft's own prices, the call costs about 3.4× what it would cost to build it from calls 103 and 104. The check e(π, [s − z]G2) = e(C − [y]G1, G2) can be rearranged as e(π, [s]G2) · e(−(C − y·G1 + z·π), G2) = 1, which needs no G2 scalar multiplication. That is one 3-point G1 MSM (about 275 µs interpolated, 4.4M gas) plus `bls12_381_pairing_check` on 2 pairs (15,590,400), about 20M in total, plus SHA-256 of 48 bytes for the versioned hash. A dedicated call priced above its own components gives services a reason to avoid it. With the calls already in the draft, it saves nothing except encoding.

**Suggested change:** Re-time the call with c-kzg-4844's `verify_kzg_proof` (blst) on the same runner and price it from that time. Otherwise, drop call 105 and specify KZG point evaluation as a composition of 103 and 104 in a "Suggestions & examples" section, with the 122 c-kzg vectors kept as test vectors for the composition.

### ZW-6

**Section:** §6 `secp256k1_recover`

**Severity:** minor

**Comment:** Matching the 0x01 precompile and accepting high s is a reasonable choice. Aztec's L1 contracts, however, verify signatures with OpenZeppelin's `ECDSA.recover`, which rejects s > n/2 (`l1-contracts/src/shared/libraries/SignatureLib.sol:25`, `src/core/libraries/rollup/ValidatorSelectionLib.sol:377`; `InvalidateLib.sol:108` uses `tryRecover`). If a settlement service uses the call as it is, it will accept the malleated twin of a signature that Aztec's Rollup contract rejects.

**Suggested change:** Add one line under *Suggestions & examples* saying that services which need OpenZeppelin semantics must check s ≤ n/2 themselves before calling. Also mention it in `docs/aztec/SERVICE.md`.

### ZW-7

**Section:** §7 `poseidon2_bn254_permute`

**Severity:** major

**Comment:** I checked the parameters against Barretenberg and **they match**. t = 4, d = 5, 8 full and 56 partial rounds (`crypto/poseidon2/poseidon2_params.hpp:19-22`). The project's constants (`client/poseidon2-bn254.json`, from which `p2` is generated) equal bb's 256 round constants exactly, and with them bb's own `TEST_VECTOR_INPUT` produces its `TEST_VECTOR_OUTPUT` (`poseidon2_params.hpp:447-458`). The spec text, however, is too loose for a consensus rule:

- "4 BN254 field elements" does not say which field. It has to be the scalar field Fr (bb's `bb::fr`, r = 0x30644e72…f0000001), not the base field.
- "The internal diagonal Noir and Barretenberg use" means different numbers in different places. bb stores μ_i = D_i − 1 (`internal_matrix_diagonal_minus_one`, lines 24-31) and computes μ_i·x_i + Σx (`poseidon2_permutation.hpp:85-98`), so the matrix diagonal is D_i = μ_i + 1 (bb's own `internal_matrix`, lines 33-58, ends in …e8 where μ_0 ends in …e7). Noir's JSON stores those same μ_i under the name `internal_matrix_diagonal`. An implementer who reads "diagonal" from the paper will be off by one on every partial round.
- The spec omits the external matrix M4 = [[5,7,1,3],[4,6,1,1],[1,3,5,7],[1,1,4,6]] (`poseidon2_permutation.hpp:45-75`), the initial linear layer applied before the first round (`:130-134`), and the fact that partial rounds use only `round_constants[i][0]` out of a 64 × 4 table (`:147`).

**Suggested change:** Write the permutation out in full in §7: the field, M4, the initial M4 layer, the internal layer as J + diag(μ) with μ listed, and the full 64 × 4 constant table as an appendix. Add bb's `TEST_VECTOR_INPUT`/`TEST_VECTOR_OUTPUT` and Noir's five permutation vectors to *Test vectors*.

### ZW-8

**Section:** §7 motivation (lines 161-164); Gas pricing (lines 221-222); Open question 1

**Severity:** major

**Comment:** The draft says that without the call "every tree update must be proven and applied in order, one package a slot", which implies the call removes that limit. It gives accumulate one depth-40 append (8.3M of 10M) and no more. Aztec's trees are not shaped like that. In v6, `NOTE_HASH_TREE_HEIGHT` = 42 and `NULLIFIER_TREE_HEIGHT` = 42 (`noir-projects/fnd/noir-protocol-circuits/crates/types/src/constants.nr:38-40`), and a transaction inserts up to 64 note hashes and 64 nullifiers (`:75-76`) as subtrees. A 64-leaf subtree append is 63 + 36 = 99 permutations, about 20.4M with the call. An indexed-nullifier insertion takes about two paths (`spikes/poseidon2-gas/RESULTS.md`), about 17M. Neither fits in 10M. The project's own recommended design (`docs/privacy/ARCHITECTURES.md`, candidate 3) keeps tree hashing in refine, where the call doubles insertions a slot. That is a real and honest motivation, and this section does not state it. The call has a second user the draft does not mention. Barretenberg's default (non-EVM) transcript is Poseidon2 (`transcript/transcript.hpp:591`, `NativeTranscript = BaseTranscript<FrCodec, Poseidon2<...>>`), so any bb proof outside the Keccak flavours draws its Fiat–Shamir challenges through this permutation.

**Suggested change:** Restate the motivation: (a) it doubles a refine-side tree package's insertions (ARCHITECTURES.md), (b) it makes accumulate able to do a single append, at most, and (c) it serves verifiers of bb's Poseidon2-transcript flavours. Drop the implication that it lifts the one-package-a-slot limit for Aztec-shaped trees.

### ZW-9

**Section:** Host-call specification, "Encodings follow Ethereum's precompiles … so their test vectors apply unchanged"; Test vectors

**Severity:** minor

**Comment:** That claim does not hold for three of the calls:

- EIP-196 has no MSM, only `ecAdd` and `ecMul`, so its vectors apply to call 101 only after a translation.
- EIP-196's `ecMul` and EIP-2537's MSM accept scalars of any 32-byte value, not only those below the group order, whereas §1 returns `HUH` for a scalar ≥ r.
- EIP-197 accepts an empty input and returns 1, whereas §2 requires k ≥ 1.

None of these matters for a verifier, since its scalars are reduced and it never pairs nothing. They do matter for "byte for byte" agreement, and for anyone reusing Ethereum's test suites.

**Suggested change:** Either reduce scalars mod r and allow k = 0, as Ethereum does, or keep `HUH` and list each divergence with the vectors that change. State the Fp2 component order for G2 explicitly (EIP-197 puts the imaginary part first) instead of relying on the cross-reference.

### ZW-10

**Section:** Gas pricing (benchmark and constants)

**Severity:** blocking

**Comment:** The constants are meant to bound time, but `spikes/jip-bench/src/main.rs` does not time the work the spec requires:

- **BN254 pairing.** `Bn254::multi_pairing` runs on pre-built affine points. It includes no byte decoding, no on-curve checks and no G2 subgroup checks, all of which §2 requires.
- **BN254 MSM.** It is the same: `G1Projective::msm` runs on affine points, with no decoding and no on-curve check.
- **BLS12-381 pairing.** It is two Miller loops on the generators and one final exponentiation, with none of the decoding or subgroup checks that EIP-2537 mandates for pairing inputs.
- **BLS12-381 MSM.** The bases are `(0..n).map(|_| g1)`, which is n copies of the generator, with patterned scalars `(i*7+3) as u8`, and no subgroup checks. The resulting slopes are implausible: on the Mac, 16 → 128 points adds 85 µs (0.76 µs a point), and on EPYC it adds 4.6 µs a point, against BN254's 20.3 µs a point on random inputs over a smaller field.

The KZG, ecrecover and Poseidon2 rows do time from input bytes, so the gap is specific to calls 101–104, which are the ones UltraHonk relies on. An underpriced host call is a cheap way to make guarantors overrun, and m = 2 was not chosen with these costs in view.

**Suggested change:** Time each call end to end, from bytes in to bytes out (or `HUH`), with every check the spec requires. Use random inputs, and also worst cases: repeated bases, points at infinity, scalars r − 1, and a malformed last item. Re-derive every G from those times before any constant is proposed.

### ZW-11

**Section:** Gas pricing table; Open question 1

**Severity:** minor

**Comment:** In the draft's own table, BN254 costs more than BLS12-381: a 2-pair pairing at 1,280 µs against 974 µs, and a 128-point MSM at 2,911 µs against 1,491 µs. BN254's base field is 254 bits and BLS12-381's is 381, so BN254 should be the cheaper curve. The difference comes from the library (arkworks against blst's assembly), not the curve. If R and m are combined with "the maximum across clients", whichever client ships the slowest BN254 library sets the price of every UltraHonk check on the network.

**Suggested change:** Benchmark more than one BN254 implementation: arkworks, Barretenberg's own `ecc` (production-audited for Aztec, with a single-threaded `pippenger`), and one more. Then set the price from a stated performance target that clients must meet, not from the slowest client.

### ZW-12

**Section:** Open question 3 (Grumpkin and the BN254 G2 MSM)

**Severity:** major

**Comment:** This can be answered now:

- **Settling Aztec does not need Grumpkin.** The root rollup verifies its children with `PROOF_TYPE_ROOT_ROLLUP_HONK`, commented "Root rollup honk proof type that does full IPA recursive verifier" (`noir-projects/fnd/noir-protocol-circuits/crates/types/src/proof/proof_data.nr:64-66`). The epoch proof that reaches L1 is therefore BN254 KZG only.
- **Checking a CHONK proof directly does need Grumpkin.** `ChonkVerifier` performs three BN254 pairing-point checks: the kernel's accumulated points, Merge, and Translator plus Joint PCS (`chonk/chonk_verifier.cpp:44-102`, aggregated into one in the recursive path, `:184-192`). It then discharges a TripleIPA claim over Grumpkin, whose cost is one MSM of 2^15 SRS points (`CONST_ECCVM_LOG_N = 15`, `constants.hpp:32`; `IPA::srs_msm`, `commitment_schemes/ipa/ipa.hpp:579-581`).
- **That 2^15 MSM amortizes.** bb reduces each proof to a deferrable accumulator and discharges many with one combined MSM (`triple_ipa.hpp:265-273`, `chonk/chonk_batch_verifier.hpp`). The ~12 billion gas per CHONK figure in `spikes/chonk` scales single-proof native time and ignores this. It is the number behind LIMITS.md's "covered by 1", and it overstates the per-proof cost for a service that checks many proofs.
- **Nothing in bb's verifiers needs a G2 MSM**, since their G2 points are fixed. With the rewrite in ZW-5, KZG does not need one either.

Grumpkin is y² = x³ − 17 over BN254's Fr, with cofactor 1 and generator (1, √−16) (`ecc/curves/grumpkin/grumpkin.hpp`). A client gets it almost free from its BN254 field code.

**Suggested change:** Answer Q3 in the text. Add `grumpkin_msm` (108) with 64-byte points and no subgroup check needed, priced with the large-n table from ZW-3. Note that a 2^15-base call passes 2 MiB of bases (a service can keep them as a preimage). Drop the BN254 G2 MSM. Re-estimate CHONK verification as per-proof reduction plus a shared IPA MSM before LIMITS.md relies on the 12B figure.

### ZW-13

**Section:** New "Rationale" section; Open question 4

**Severity:** minor

**Comment:** The draft's best design decision is not explained anywhere: primitives, not verifiers, and whole operations, not field arithmetic. Barretenberg's proof format has changed at every major release. bb 6 alone changed how challenges are derived (whole Keccak output, gamma = H(beta)) and added the ROM-LogUp subrelations, 29 → 31 (`spikes/epoch-proof/RESULTS.md`; `docs/privacy/LESSONS.md` 3e). The curves did not change. A `verify_ultrahonk` host call would need a protocol amendment for every bb release. Field-arithmetic calls would not pay either. Poseidon2, which is pure field arithmetic, already runs within f = 4.7 of native in the recompiled PVM. A host-call boundary for each multiplication cannot beat that, and sumcheck is ≤5% of the verifier anyway (ZW-1).

**Suggested change:** Add a short Rationale making both points. Answer Q4: keep separate indices, as JIP-1 does, since a selector adds a failure mode and saves nothing. Note that the call surface matches zkVerify's `CurveHooks`, so existing arkworks-based verifiers adopt the calls by implementing a single trait.

### ZW-14

**Section:** Motivation (sources for 812M and 981M)

**Severity:** minor

**Comment:** The draft's 812M is correct (180.8M + 199.5M + 432.8M = 813.0M), but one of the documents it cites is stale. `docs/aztec/SERVICE.md:68-69` gives "an epoch proof is about 369M plus its attestations". That figure comes from `spikes/kzg-gas/RESULTS.md`'s "epoch proof (about 169M at 2^24)", which predates the measured 180,791,299. It should be about 380M.

**Suggested change:** Cite `spikes/ecdsa-gas/RESULTS.md`, where the 812M and 981M sums are worked out. Correct the 169M and 369M figures in the two cited files so that a reviewer who follows the links finds the same numbers as the draft.
