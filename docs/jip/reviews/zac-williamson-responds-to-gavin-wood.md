# Response to the Gavin Wood review of JIP-8 (draft at 4f0fd19)

*Simulated review: written by an AI in the persona of Zac Williamson for internal QA. It is not Zac Williamson's view and has not been seen by him.*

This responds to `docs/jip/reviews/gavin-wood.md`, comments GW-1 to GW-14, and cross-references my own review, `docs/jip/reviews/zac-williamson.md` (ZW-1 to ZW-14). Unless they start with `docs/`, `spikes/` or `text/` (the Gray Paper 0.8.0 checkout, `submodules/graypaper`), paths are relative to `aztec-packages/` at v6.0.0.

## GW-1

**Position:** agree in part

I agree that the draft needs an "Alternatives considered" section with numbers. GW's reading of the cost model is accurate. `mul_upper_u_u` costs 4 cycles and 4 decode slots (`text/pvm.tex:1382`), the initial unit vector is ⟨4, 4, 4, 1, 1⟩ (`text/pvm.tex:1004`), and the GP leaves cryptographic instructions out on purpose (`text/overview.tex:145`).

I don't accept the sequencing, though: that host calls should be argued for only after the ISA route has been shown to fail. The two routes address different parts of the cost.

- **Pairings and MSMs.** Here the gap is algorithmic as well as instruction-level: precomputed Miller lines (`ecc/curves/bn254/pairing.hpp:28`, `commitment_schemes/verification_key.hpp:61-62`), endomorphisms, and Pippenger tuning. A better multiply instruction does not bring those into the PVM.
- **Field arithmetic that no call touches.** In ZW-2 I estimated that about 380M gas of every Aztec checkpoint is BLS12-381 scalar-field arithmetic: the 4,096-point blob evaluation plus its Fiat–Shamir hash. A widening multiply-accumulate or add-with-carry is exactly what would help there, and so would sumcheck and Poseidon2.

**Reconciliation with ZW-2 and ZW-13:** treat the ISA work as complementary, not as a gate. Run GW's experiments (a)–(c). I would add the blob evaluation kernel to (a), since it is the largest cost no call covers. Report both routes side by side.

## GW-2

**Position:** agree in part

GW is right that the GP has no "reference machine". He is also right that tying prices to one client's recompiler is perverse: a faster PolkaJam raises R, and with it every cryptographic price. His arithmetic checks out:

- **Refine.** 5e9 gas over 1.5–3 s (25–50% of a 6 s core, `text/discussion.tex:77`) is 1.67–3.33 gas/ns.
- **Accumulate.** 10M gas over 10 ms (`text/discussion.tex:79`) is 1 gas/ns.
- **Ratio.** R = 8 is 2.4–4.8× above the refine figure.

This matters more than his review spells out. The saving is K / (R_d · m), where K = R · f is PVM gas per native nanosecond (x86: KZG about 47, secp256k1 about 65, Poseidon2 about 33, from `spikes/jip-bench/RESULTS.md`). With the GP's own sizing, R_d = 1.67–3.33 at m = 2, the savings are KZG 7–14×, secp256k1 10–19× and Poseidon2 5–10×, not the draft's 2–4×.

The anchor is a real choice, though, not a correction. At R_d, a package of host calls may run 2.4–4.8× longer than ordinary PVM code of the same gas does on PolkaJam. That is still inside the GP's stated budget, but it is no longer at parity with how PVM code is actually charged.

**Reconciliation with ZW-8:** my objection that Poseidon2 still leaves accumulate short assumed the draft's R = 8. At R_d, one permutation is 12.9 µs × 1.67–3.33 × 2 = 43k–86k gas. A depth-42 path is then 1.8–3.6M, and a 64-leaf Aztec subtree append (99 permutations) is 4.3–8.5M, which fits in 10M. So the invariant GW asks for decides whether my comment stands. Both reviews should ask the draft to state that invariant first.

## GW-3

**Position:** agree

This reinforces ZW-10, and GW adds three points I missed: R taken from a total rather than a slope, medians used where a bound needs a high percentile, and Poseidon2 timed as the sponge. I checked his slope. (432,756,815 − 13,119,449) / (62.379 − 8.374) ms = 419,637,366 / 54.005 ms = **7.77 gas/ns**, which is correct, and the implied fixed cost is 8.374 − 1.689 = 6.69 ms.

One correction: that fixed cost belongs to that service and does not carry over to the others. The whole 100-hash Poseidon2 package ran in 6.130 ms, which is less than 6.69 ms. Subtracting secp256k1's fixed cost from KZG's 25.3 ms is therefore not valid, and "KZG's 7.9 is also understated" is likely but not shown. GW's own remedy, a slope from two package sizes per kernel, is the right fix.

On item 5: Noir's sponge hashes two elements with one permutation plus a little overhead, and the 428,839-gas PVM baseline is the same `p2::hash`. The effect is small and errs towards overcharging, but the call should still be timed as specified.

## GW-4

**Position:** agree

This is the same mismatch as ZW-3 (an affine formula in §§1 and 3, interpolation in the pricing section), and both reviews propose an EIP-2537-style discount table. GW's overflow point is new and correct. With φ8 up to 2^64 − 1, wrapping u64 and unbounded integers give different outcomes, and work results distinguish the two (`text/reporting_assurance.tex:117`). Capping φ8 at the 2^32 address space and fixing the order of evaluation (gas, then memory, then validation) removes the ambiguity. The missing second BLS pairing size (open question 5) has to be measured before the call has a price.

## GW-5

**Position:** agree

This reinforces ZW-9 and goes further. The extra divergences GW lists are real. EIP-4844's precompile fails on a versioned-hash mismatch and returns 64 bytes (FIELD_ELEMENTS_PER_BLOB, BLS_MODULUS) on success. The 0x01 precompile returns a 32-byte left-padded word, or empty output. The draft's failure list for §6 omits r not being an x-coordinate on the curve. JAM-native vector files covering every `HUH` branch, plus a mapping table to each EIP's outcomes, are the right deliverable.

## GW-6

**Position:** agree

This is the same finding as ZW-5, reached independently, and GW's figures are right. The identity e(C − [y]G1 + [z]π, −G2) · e(π, [s]G2) = 1 follows from e(π, [s − z]G2) = e(C − [y]G1, G2). The 3-point MSM interpolates to 168.0 + 2 × 53.6 = 275.2 µs, which is 4.40M gas. Adding call 104's 15,590,400 gives about 20.0M, against G_KZG = 67,888,000, so 3.4×.

One addition: the composition still has to supply [s]G2 and −G2 as pairing inputs and pay their G2 subgroup checks on every call. The fixed-G2 variant I suggested for BN254 (ZW-4) would remove that cost here too. Dropping call 105 does not strand Aztec: its `FinalBlobAccumulator` still commits by BLS12-381 KZG (`docs/aztec/BLOBS.md`), and 103 + 104 verify it.

## GW-7

**Position:** agree in part

On the motivation I agree; it is ZW-8's point. The draft's "one package a slot" sentence overreaches, and the honest case is refine throughput under ARCHITECTURES.md's candidate 3. Two of GW's specifics need correcting.

- **The frontier I/O is not decisive.** Forty `read` calls at about 4.4k gas each (2407 + 1736 + 248, `text/definitions.tex:331-333`) plus forty `write` calls at about 6.0k (2442 + 216 + 3358, `:337-339`) come to about 0.42M. A depth-40 append is then about 8.7M, still under 10M.
- **"One project's round constants" overstates it.** Barretenberg's parameters are generated by Horizen Labs' reference sage script (`crypto/poseidon2/poseidon2_params.hpp`, header) and differentially tested against an independent EVM implementation (`crypto/poseidon2/README.md`, zemse/poseidon2-evm). This is the standard BN254 t = 4 instance. It is also Barretenberg's default transcript hash (`transcript/transcript.hpp:591`) and Aztec's tree hash.

Where I part company is removal on the grounds that the saving is the smallest (2.1×). That figure depends on the anchor: under GW-2's own GP-anchored pricing, Poseidon2 saves 5–10×.

**Reconciliation:** I can accept moving Poseidon2 into a separate JIP (GW-11), provided it proceeds in parallel. It should carry a refine-throughput measurement of candidate 3, constants stated by value in the text (ZW-7), and a qualification policy that answers open question 2.

## GW-8

**Position:** agree

This is outside what I reviewed, but GW's grounding holds. Refine's view of state is anchored to the package's lookup-anchor slot (`text/pvm_invocations.tex:98`, `historical_lookup` taking `lookupanchortime`). Is-authorized takes only (p, c) (`:37`). An unknown call charges 1000 gas (`text/definitions.tex:306`) and returns `WHAT` (`text/pvm_invocations.tex:52-53`). Activation tied to the executing node's slot would make guarantor and auditor results diverge. Tying it to the work-package context, and defining a probe whose result is fixed both before and after activation, are both needed.

## GW-9

**Position:** agree in part

I agree with normative text, conformance vectors and differential fuzzing across two or more independent implementations. ZW-7 makes the same demand for Poseidon2's constants, and ZW-11 for prices that don't follow the slowest library. GW is right that BLS12-381 is already in the GP (`text/beefy.tex:4`) and that BN254 and secp256k1 are not.

My reservation is about the dilemma as he frames it. BN254 already has several mature, independently written implementations: arkworks, Barretenberg's `ecc`, gnark-crypto and mcl among them. The realistic path is two or more of those, fuzzed against each other, not clean-room pairing code in every client. Clean-room pairing code is where I would expect the edge-case divergences GW warns about.

## GW-10

**Position:** agree in part

The suggested change is ZW-1's: stub out the curve operations and report the projected gas. The reference verifier already sends them through `CurveHooks` (`ultrahonk_verifier/src/lib.rs:570`, `:599`), so this is one experiment.

GW's Amdahl concern, though, is not borne out by the repository's numbers. The verifier runs sumcheck first and rejects there (`lib.rs:127-137`). A rejected 2^24 proof costs 9,831,418 gas against 180,791,299 accepted, so transcript plus sumcheck are about 5%. Of the per-doubling increase, only about 0.2M is sumcheck (rejection gas from 2^23 to 2^24: 9,631,420 → 9,831,418). The other ~1.4M is Shplemini, including one more MSM point. At the draft's prices the projection is about 55–65M, roughly 3×, not "well below 2×". I agree the motivation should lead with that number and move the 50× comparison into context.

## GW-11

**Position:** agree in part

I support the split and the "Process" paragraph. `docs/jip/FORMAT.md` confirms that no merged JIP changes a GP rule, so where the normative text lands has to be stated. Both reviews answer open question 4 the same way: separate indices.

I would not accept a core that permanently excludes Grumpkin. Checking a CHONK proof directly needs a 2^15-point Grumpkin MSM (`constants.hpp:32`, `commitment_schemes/ipa/ipa.hpp:579-581`), which batches across proofs (`commitment_schemes/triple_ipa/triple_ipa.hpp:265-273`). Grumpkin is nearly free to implement on BN254's Fr (`ecc/curves/grumpkin/grumpkin.hpp`). Settling Aztec doesn't need it (`noir-projects/fnd/noir-protocol-circuits/crates/types/src/proof/proof_data.nr:64-66`).

**Reconciliation with ZW-12:** the core JIP is GW's (BN254 and BLS12-381 pairing plus G1 MSM). Grumpkin MSM becomes a follow-on JIP with its own measured case, which fits GW's "each with its own measured case". On the "precompile catalogue" worry: the curves have stayed fixed across bb 3 → 5 → 6 while the proof format changed every time (`docs/privacy/LESSONS.md` 3e). Curve primitives age far better than verifier calls.

## GW-12

**Position:** agree

Returning the 64-byte public key is more general, and the address costs the service one Keccak, which the GP already defines. Aztec's use only needs the address: OpenZeppelin's `ECDSA.recover` returns one (`l1-contracts/src/shared/libraries/SignatureLib.sol:25`). This is compatible with ZW-6: whichever output is chosen, the service must still enforce s ≤ n/2 itself to match Aztec's contracts.

## GW-13

**Position:** agree

The figures are right. G_I = 50M (`text/definitions.tex:266`), and Ψ_I dispatches only `gas`, `grow_heap` and `fetch` (`text/pvm_invocations.tex:43-50`). G_KZG at 67.9M exceeds G_I, and a 2-pair BN254 check (20,472,000) is 41% of it. Composed KZG (about 20M, GW-6) would fit, but no authorizer use case is offered. Exclude Ψ_I until one is.

## GW-14

**Position:** agree

All four items check out:

- The draft's list has 26 names. `grow_heap` = 1 and `historical_lookup` = 7 are missing (`text/pvm_invocations.tex:311`, `:547`), and `provide` = 27 (`:1018`).
- The unknown-call charge is 1000 gas (`text/definitions.tex:306`).
- `spikes/jip-bench/RESULTS.md` says the KZG gas "differs from the local 199,453,527", yet both of its tables print 199,453,527.
- GW's list of what the draft gets right also holds: charging before execution matches block-level pre-charging (`text/pvm.tex:158`).

## Where we agree

Revisions I believe both reviewers would support:

1. **State the pricing invariant and anchor it in the GP's unit,** not one recompiler's wall-clock R. Then recompute every saving under it, because the anchor moves the headline from 2–4× to 5–19× (GW-2, ZW-3, ZW-8).
2. **Re-benchmark end to end:** from encoded bytes, with every decoding, on-curve and subgroup check the spec requires, on random and adversarial inputs, with R measured as a per-kernel slope and prices set from high percentiles. Use at least two independent implementations per call (GW-3, GW-9, ZW-10, ZW-11).
3. **Write one normative gas schedule per call** in the spec section itself: an EIP-2537-style discount table for MSMs, φ8 capped at 2^32, a fixed order of evaluation, and the second BLS pairing size measured (GW-4, ZW-3).
4. **Drop `kzg_point_evaluation`** and publish the 103 + 104 composition (about 20M against 67.9M) with the EIP-4844 vectors as its test (GW-6, ZW-5).
5. **Measure the UltraHonk verifier with the curve operations stubbed,** and lead the motivation with the projected gas (about 3× by our estimate) instead of the 50× share comparison (GW-10, ZW-1).
6. **Stop relying on "vectors apply unchanged".** List every divergence from the EIPs, or adopt their semantics, and ship JAM-native vectors for every `HUH` branch (GW-5, ZW-9).
7. **Restate Poseidon2's motivation** as refine throughput and write its constants out by value. Whether it stays in this JIP or moves to its own, it should not claim to lift the one-package-a-slot limit (GW-7, ZW-7, ZW-8).
8. **Split the scope:** a core JIP of BN254 and BLS12-381 pairing plus G1 MSM, with separate indices, and other calls (secp256k1, Poseidon2, Grumpkin) proposed on their own measured cases. Add a Process paragraph saying where the normative text lands (GW-11, ZW-12, ZW-13).
9. **Fix activation and detection:** key activation to the work-package context for refine, exclude is-authorized until a use case exists, and define a support probe (GW-8, GW-13).
10. **Fix the factual slips:** the host-call list, "reference machine", the 1000-gas `WHAT`, the RESULTS sentence, and the stale 169M and 369M figures (GW-14, ZW-14).
