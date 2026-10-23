# Response to the simulated Zac Williamson review of JIP-8 (draft at 4f0fd19)

*Simulated review: written by an AI in the persona of Gavin Wood for internal QA. It is not Gavin Wood's view and has not been seen by him.*

The two reviews start from different places. Zac's asks whether the calls are shaped right for the proof systems that will use them; mine asks whether they belong in the protocol at all. They agree more than that framing suggests. Where they conflict, it is mostly over scope: Zac would add calls (a fixed-G2 pairing, Grumpkin, possibly a blob batch), and I would remove some (KZG, Poseidon2). I checked his figures against the repository, `ultrahonk_verifier` (ae599d3) and `aztec-packages` (checked out at `v6.0.0-nightly.20261021`). The arithmetic holds, with one inference I dispute (ZW-3.3). Where the repository has moved since 4f0fd19, I say so.

## ZW-1

**Position:** agree

The split is sound, and it is the measurement I asked for in GW-10, nearly done already. I checked the figures. `verify_sumcheck` returns before `verify_shplemini` (`ultrahonk_verifier/src/lib.rs:127–137`). A rejected 2^24 proof costs 9,831,418 gas against 180,791,299 accepted (`spikes/epoch-proof/RESULTS.md`), so about 5.4% of the cost lies before Shplemini. The MSM size is `number_of_unshifted + log_n + 2` (`lib.rs:330–336`). A 64-point BN254 MSM interpolates to 633.2 + 48 × 20.34 µs = 1,609.5 µs, which is 25.75M gas at 16 gas/ns. The 2-pair pairing is 20,472,000. So "55–65M, about 3×" follows from the draft's constants. Two caveats from my side. First, the estimate is a floor: calls 101–102 were timed without decoding or subgroup checks (GW-3, ZW-10), so their real G is higher. Second, it inherits the R = 8 anchor, which GW-2 disputes; at a cycle-based anchor the gain would be larger. Reinforces GW-10. Measure it with a zero-gas `CurveHooks` build, as he suggests, and report it under both pricing anchors.

## ZW-2

**Position:** agree in part

The "with JIP-8" column is right, and his estimates check out. Epoch: 60M + 67.9M + 33 × 3,216,000 (106.1M), about 234M, or 186M with KZG composed, which is 3.5–4.4× against 813M. Checkpoint: 262.6M − 199.5M = 63.1M of hashing and evaluation per blob that no call touches, 6 × 63.1M ≈ 379M, so about 400–420M against 981M. I disagree with the first option he offers, a whole-operation call for `verify_blob_kzg_proof_batch`. That is the precompile catalogue GW-11 warns against, and it bakes Ethereum's blob format (4,096 BLS12-381 scalars, Ethereum's Fiat–Shamir domain) into JAM's consensus. The residue he identifies, SHA-256 over 128 KiB plus a barycentric evaluation, is plain field arithmetic and hashing. That is exactly what GW-1's instruction-level route would cheapen for everyone. Take his second option: state that blob evaluation stays in the PVM, and point at `docs/aztec/BLOBS.md`'s alternative of letting JAM's own availability stand in for the KZG binding. This conflicts with GW-1 and GW-11 only on the first option.

## ZW-3

**Position:** agree in part

Points 1 and 2 are the same as GW-4: one normative formula, tabulated, EIP-2537 style. On large n, linear extrapolation overcharges. That is the safe direction, but it should not be the specified one, and a measured table up to the gas-feasible n settles it.

Point 3's inference does not hold, though its suggested check is right. His figures are exact: (180,791,299 − 179,184,883) − (9,831,418 − 9,631,420) = 1,606,416 − 199,998 = 1,406,418. But that is the marginal cost of one more point inside a ~62-point Pippenger MSM (plus Shplemini scalars), not a standalone scalar multiplication. The draft's own native data show how far apart the two are: arkworks' marginal point between 16 and 128 costs 20.3 µs, against 131.2 µs for a 1-point call, about 6.5×. A PVM standalone scalar multiplication is therefore plausibly well above 1.4M, and G(1) = 2,099,200 may well be below it. Measure a standalone PVM BN254 scalar multiplication and require G(1) to sit below it, as he proposes. Reinforces GW-4.

## ZW-4

**Position:** disagree

A fixed-G2 variant buys gas only if the protocol names the points, and here those points are [1] and [x] from Aztec's BN254 setup. That puts a second external trusted setup into JAM's consensus, for one proof-system family. His argument that "the KZG call already builds in [s]G2" is, for me, a reason to remove call 105 (GW-6), not a precedent to extend. A client is free to cache prepared Miller lines for G2 points it sees repeatedly. But gas must not depend on a cache, so the call is priced for the uncached case either way. The underlying concern is real: per-call G2 subgroup checks and line computation may be a large share of a 2-pair check. GW-3's end-to-end benchmark will show how large. If it turns out to dominate, the general answer is a measured per-pair term, not named points. This conflicts with GW-6. I would reconcile by measuring first and deciding second, with any fixed-point call proposed separately.

## ZW-5

**Position:** agree

This is GW-6, reached independently and with the same figures: a 3-point MSM at about 275 µs (4.4M), plus 15,590,400 for the pairing, about 20M against 67,888,000, so 3.4×. On his two options, re-timing with c-kzg-4844 would bring G_KZG close to the composition's cost. The call would then save little beyond encoding, which is his own observation, and it would still carry Ethereum's versioned-hash format and ceremony into the GP. I prefer his second option: drop 105, publish the composition under "Suggestions & examples", and keep the 122 c-kzg vectors as its tests. Reinforces GW-6.

## ZW-6

**Position:** agree

I checked it: `SignatureLib.sol:25` calls OpenZeppelin's `ECDSA.recover`, which rejects high s. The call should keep the 0x01 semantics, which are the more general of the two, and the note belongs in the text exactly as he suggests. It complements GW-12. Whether the call returns an address or a public key, s is in the caller's own input, so the low-s check costs the service a single comparison.

## ZW-7

**Position:** agree

On a single point, the parameters (`poseidon2_params.hpp:19–22`: t = 4, d = 5, 8 full and 56 partial rounds, over `bb::fr`), we checked the same lines. The rest is the level of precision a consensus rule needs and the draft lacks: which field, M4, the initial linear layer, the D_i = μ_i + 1 convention, and the partial-round constant indexing. It is the concrete form of GW-7's "constants stated normatively in the text". That does not change my view that call 107 should leave this JIP (ZW-8 below). But wherever it ends up, it should be specified to this standard, and his list is the checklist. Reinforces GW-7.

## ZW-8

**Position:** agree in part

I agree with every fact, and I checked them: `NOTE_HASH_TREE_HEIGHT = 42`, `NULLIFIER_TREE_HEIGHT` the same, 64 a transaction (`constants.nr`), and `NativeTranscript` on Poseidon2 (`transcript.hpp:591`). The arithmetic is right: 63 + 36 = 99 permutations × 206,400 = 20.4M for a 64-leaf subtree, and about 17M for an indexed-nullifier insertion. Neither fits 10M. So the draft's "without it, every tree update must be proven and applied in order" is wrong on both reviewers' readings.

Where we part is his restated motivation (a). Since 4f0fd19 the repository has measured the refine-side append (commit 429c198, `spikes/poseidon2-gas/RESULTS.md`): 438,966 gas a note, 1.02 hashes, so 11,350 notes in one full package. That is against 29 UltraHonk checks a package for verification. Insertion is no longer the bound, so doubling it buys a pool nothing until verification is about 390× cheaper. That leaves (c), verifiers of bb's Poseidon2-transcript flavours, as the one live motivation. That is CHONK-type proofs, whose direct check is itself unsettled (ZW-12). Reconciled position: take call 107 out of this JIP, and let it return with a measured (c).

## ZW-9

**Position:** agree

This is GW-5's list, and our citations match. On the remedy, I would rather adopt the EIP semantics outright: reduce scalars mod r and accept k = 0 returning 1. Every departure is a vector someone has to rewrite and a place where clients can diverge, and a verifier never notices the difference, as he says. The Fp2 component order (imaginary part first in EIP-197) should be stated, not left to a cross-reference. Reinforces GW-5.

## ZW-10

**Position:** agree

This is GW-3, items 2–3, with the same evidence. The Mac figures check: (422.5 − 337.0) / 112 = 0.76 µs a point. I accept his severity, blocking, over my "major" for the constants specifically: no G should be proposed from these timings. My GW-3 adds two items his doesn't cover. R is taken from total refine time including fixed costs (the marginal secp256k1 R is 7.77, not 6.9), and medians come from a shared VM. Reinforces GW-3.

## ZW-11

**Position:** agree

This is the library-dependence problem in GW-2 and GW-9 made concrete. BN254 priced above BLS12-381 is an artefact of library choice, not of the curves. His remedy, "a stated performance target that clients must meet, not the slowest client", is the same as GW-2's: price from a reference implementation on a named reference microarchitecture, in the GP's unit. Benchmarking Barretenberg's own `ecc` alongside arkworks is a good way to set that reference. Reinforces GW-2.

## ZW-12

**Position:** agree in part

Agreed: answer Q3 in the text. Settling Aztec does not need Grumpkin; I checked `proof_data.nr:64–66`, the root rollup's full IPA recursive verification. Drop the BN254 G2 MSM. Re-estimate CHONK with amortised IPA before `docs/privacy/LIMITS.md` relies on 12B. That last point corrects a figure the draft's motivation leans on indirectly. The 2^15 bases come to 2 MiB (32,768 × 64 bytes), as he says.

Not agreed: adding `grumpkin_msm` (108) now. By his own analysis, its only consumer is a direct CHONK check whose cost is about to be re-estimated. Adding a curve on that basis is how seven calls become twelve. It also illustrates GW-1. Each new curve someone needs (Grumpkin today, Pasta or BabyJubJub tomorrow) is another argument for the instruction-level route, which serves all of them without a JIP each. Reconciled: Q3's answer goes in the text, with Grumpkin named as a candidate for a later JIP once the amortised CHONK cost is measured.

## ZW-13

**Position:** agree in part

On primitives, not verifiers, we fully agree. The bb 5 → 6 format churn (`docs/privacy/LESSONS.md` §3e) is the strongest argument against a `verify_ultrahonk` call, and the draft should say so. On separate indices for Q4, we agree too (GW-11). The `CurveHooks` note is useful.

Where I differ is the inference that field arithmetic is settled. His argument shows that field-arithmetic *host calls* would not pay, because the per-call boundary costs more than f = 4.7 recovers. GW-1 proposes *instructions*, a widening multiply or add-with-carry, or a re-costed `mul_upper_u_u` (`pvm.tex:1382`, 4 decode slots), which have no host-call boundary. The "95% in curve operations" he finds in ZW-1 does not weaken that case, because MSMs and pairings are multi-limb field multiplications all the way down. The Rationale he proposes should therefore say "primitives, not verifiers; whole operations, not field-arithmetic host calls", and leave the ISA question to GW-1's "Alternatives considered".

## ZW-14

**Position:** agree

Correct at 4f0fd19: `docs/aztec/SERVICE.md:69` read "about 369M" and `spikes/kzg-gas/RESULTS.md:48–49` "about 169M … 369M". The uncommitted working tree now says 380M and 180,791,299 in both files, with SERVICE.md citing `spikes/ecdsa-gas/RESULTS.md`, so his fix is under way. One small addition: his paths say aztec-packages v6.0.0, while the local checkout is tagged `v6.0.0-nightly.20261021`. The lines I checked match, but the citation should give the exact ref.

## Where we agree

Revisions I believe both reviewers would support:

1. **Re-benchmark before proposing any constant.** End to end from bytes, with every decoding, on-curve and subgroup check the specification requires; random and adversarial inputs; worst cases reported (GW-3, ZW-10).
2. **One normative, tabulated MSM schedule** in §1 and §3, measured up to large n, with G(1) checked against a measured standalone PVM scalar multiplication (GW-4, ZW-3).
3. **No dedicated KZG call at its current price.** Compose point evaluation from calls 103 and 104, keep the c-kzg vectors as tests of the composition, and drop call 105 (GW-6, ZW-5).
4. **Explicit semantics and JAM-native vectors.** Remove "their test vectors apply unchanged", adopt or tabulate the EIP semantics per call, and state the Fp2 order (GW-5, ZW-9).
5. **Measure the gain where it matters.** A zero-gas `CurveHooks` build of the UltraHonk verifier, plus "with JIP-8" estimates for the epoch and checkpoint rows, leading the motivation instead of the 50× comparison (GW-10, ZW-1, ZW-2).
6. **Correct Poseidon2's motivation.** It does not lift the one-package-a-slot limit for Aztec-shaped trees. If the call survives anywhere, specify the permutation in full (GW-7, ZW-7, ZW-8).
7. **Price from a stated reference, not the slowest client or library** (GW-2, ZW-11).
8. **A Rationale section and separate indices.** Primitives, not verifiers; whole operations, not field-arithmetic host calls; Q4 answered as separate indices (GW-11, ZW-13).
9. **Answer Q3 in the text.** No BN254 G2 MSM; Aztec settlement needs no Grumpkin; re-estimate CHONK with amortised IPA before relying on 12B (ZW-12, and GW-11's scope concern).
10. **The small corrections:** the high-s note for OpenZeppelin semantics (ZW-6), stale citations (ZW-14), and the host-call list and "reference hardware" wording (GW-14).
