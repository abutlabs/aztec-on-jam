# Review: JIP-8 draft, Cryptographic host calls (`docs/jip/jip-draft.md` at 4f0fd19)

*Simulated review: written by an AI in the persona of Gavin Wood for internal QA. It is not Gavin Wood's view and has not been seen by him.*

## Overall assessment

The draft is honest about what it measures, and that honesty is its best feature: it says up front that a host call priced consistently with PVM code saves about 2–4×, not the 50× the Ethereum comparison suggests. But that same finding is the strongest argument against the proposal in its present form. A permanent, consensus-critical addition of seven calls, three curves, one hash parameterisation and one external trusted setup, to buy 2–4×, needs to show that nothing more general (the PVM's instruction set, its gas cost model, better code inside the PVM) gets there first, and it does not try. The pricing rule is anchored to one client's recompiler on one shared CI machine, rather than to the Gray Paper's own unit of gas, and the benchmark behind it skips work the specification requires. Several statements contradict either the draft's own measurements or the precompiles it says it follows. I would not take this to the Fellowship yet. I would take a much smaller version: BLS12-381 and BN254 pairing and G1 MSM only, priced in the GP's unit, with alternatives measured.

## Comments

### GW-1: The case against doing this in the PVM is not made

- **Section:** Motivation; Gas pricing ("What this means for the gain")
- **Severity:** blocking
- **Comment:** The GP leaves cryptographic instructions out of the PVM on purpose: "the complex instructions for cryptographic operations are missing as are those which deal with environmental interactions. Overall it is far less opinionated" (`overview.tex:145`). A JIP that puts them back has to show the general route fails. The draft's own number, f = 4.7–9.4 at m = 2, giving 2–4×, is the size of gain an ISA or cost-model change could plausibly reach, and that change would help every field and curve: Grumpkin, Pasta, Goldilocks, the ones in Open questions 2–3, and the ones nobody has thought of yet. The GP gives grounds to look there first. A full 64×64→128 product takes `mul_64` (3 cycles, one MUL unit) plus `mul_upper_u_u` (4 cycles, **4 decode slots**, one MUL unit) (`pvm.tex:1377`, `pvm.tex:1382`). The modelled core has a single MUL unit and 4 decode slots a cycle (initial units ⟨4,4,4,1,1⟩, `pvm.tex`, eq. `gascostforblock`). There is no add-with-carry, so carries cost compare instructions at 3 decode slots each (`regcomp`, `pvm.tex` cost tables). Multi-limb field arithmetic is the worst case for this model. The draft also counts "library gain (blst's assembly against the pure-Rust code services run)" towards the case for host calls (Gas pricing). Part of that gain is algorithmic, and services could have it inside the PVM today.
- **Suggested change:** Add an "Alternatives considered" section with numbers. (a) Profile the KZG, secp256k1 and Poseidon2 kernels under the GP cost model: what share of gas goes to `mul_upper_*`, `mul_64` and carry compares. (b) Estimate the gas with a hypothetical widening multiply-accumulate or add-with-carry instruction, or with `mul_upper_u_u` re-costed. (c) Compile a stronger implementation to PVM (blst's portable C path, or arkworks with its best backend) to separate algorithm from native code. Argue for host calls only for the residue that (a)–(c) cannot reach.

### GW-2: The pricing rule is anchored to the wrong reference

- **Section:** Gas pricing (G = ⌈t_native × R × m⌉)
- **Severity:** blocking
- **Comment:** The GP has no "reference hardware" or "reference machine", which the draft invokes twice. Its gas is defined as virtual CPU cycles of a modelled out-of-order core (`pvm.tex`, §Gas Cost Model, eq. `gascostforblock`). Its budgets are sized against "a regular CPU core executing at somewhere between 25% and 50% of regular speed for the whole six-second portion", with 10 ms for accumulation (`discussion.tex:77–79`). The only hardware statement is an informal "modern 16 core CPU" (`discussion.tex:14`). R, by contrast, is PolkaJam's measured wall-clock throughput. That has two consequences:
  1. Host-call prices become a function of one implementation's recompiler. If a client team makes the recompiler faster, R rises, and so does the gas every cryptographic call costs. That is perverse, and nothing the GP could state normatively.
  2. The anchor decides the result. Measured against the GP's own sizing, 5e9 gas over 1.5–3 s of core time is about 1.7–3.3 gas/ns (accumulate: 10M gas over 10 ms, 1 gas/ns). R = 8 is therefore 2.4–4.8× more conservative than the GP's design, and the headline "2–4×" is largely a product of that choice. Being conservative is not wrong, but the draft has to choose its invariant knowingly, and the draft never states one. "So that a package's gas still bounds its time" does not say whether the bound is against the worst-case PVM code or against the GP's budget.
- **Suggested change:** State the invariant explicitly. Express prices in the GP's unit, as cycles of the reference implementation on a named reference microarchitecture times m, so that they do not depend on any PVM implementation's speed. Say how this relates to the method behind the existing host-call constants (`definitions.tex:306–368`, e.g. `read` 2407, `write` 2442). If the method differs, explain why cryptographic calls deserve a different one.

### GW-3: The benchmark omits work the specification requires, and R is biased low

- **Section:** Gas pricing ("R, measured", constants table); `spikes/jip-bench`
- **Severity:** major
- **Comment:** Five problems in `spikes/jip-bench/src/main.rs` and `pricing.py`:
  1. **R includes fixed costs.** `pricing.py` takes PolkaJam's `RefineCost total` ns, which includes program setup and the package's fixed overhead. The draft drops the single recovery for this reason but keeps the others. From the draft's own secp256k1 rows, the marginal figures are (432,756,815 − 13,119,449) gas over (62.379 − 8.374) ms, which is **7.77 gas/ns**, not 6.9. The fixed cost is about 6.7 ms, more than a quarter of KZG's 25.3 ms total, so KZG's 7.9 is also understated. Under this formula, understating R underprices the call.
  2. **The curve calls are timed as bare kernels.** Points are generated in memory as affine values, so nothing is decoded and no on-curve or subgroup check runs. The specification requires those checks (`HUH` on a G2 point outside the subgroup; EIP-2537's MSM requires a G1 subgroup check on every point). KZG and ecrecover, by contrast, are timed end to end from bytes. The per-point subgroup checks scale with n and are unmeasured; on the BLS12-381 MSM they could plausibly exceed the m = 2 margin.
  3. **The inputs are degenerate.** The BLS12-381 MSM uses n copies of the generator with patterned scalars, and the BLS pairing pairs the generators with themselves. Neither is random, and neither is adversarial.
  4. **The statistics are wrong for a bound.** The constants rest on medians of 25 runs, from one run, on a shared 4-vCPU VM with SMT and unknown frequency scaling. A price that must bound time needs a high percentile or the maximum.
  5. **The Poseidon2 timing is the sponge.** It times `p2::hash` of two elements, not the bare permutation that call 107 specifies.
- **Suggested change:** Time every call end to end from its encoded input, with every validity check the specification demands. Use random inputs and adversarial ones (identical points, P with −P, infinity, maximal scalars), and report p99 and the maximum. Measure R as a slope (two package sizes) and not as a total. Run on pinned-frequency bare metal, with at least two independent implementations per call.

### GW-4: The gas schedule as specified cannot be implemented, and contradicts itself

- **Section:** Host-call specification §§1, 3, 4; Gas pricing; Open question 5
- **Severity:** major
- **Comment:**
  - §1 and §3 give an affine schedule, G_BASE + n·G_ITEM. The pricing section instead says MSM gas is "interpolated linearly between the measured sizes (1, 16 and 128 points), and extrapolated past 128". These are different functions, and neither has its constants written down.
  - G_BLS_PAIR_ITEM is unmeasured (Open question 5), so call 104 has no price.
  - Overflow is unspecified. With φ8 up to 2^64 − 1, n·G_ITEM exceeds 2^64 by orders of magnitude, since G_ITEM is in the hundreds of thousands. A client computing in wrapping u64 charges little and then panics on memory. A client computing in ℕ runs out of gas. Work results distinguish the two (∞ against ☇, `reporting_assurance.tex:117`), so the package's report differs between them.
- **Suggested change:** Give one closed-form schedule per call in the GP's own form: constants named M_□,c and M_□,item in `definitions.tex` style, or an EIP-2537-style discount table if the MSM's sub-linearity matters. Specify the evaluation order (the gas check in unbounded integers first, then memory accessibility, then validation) and cap φ8 at the 2^32 address space. Measure the second BLS pairing size before submitting.

### GW-5: "Their test vectors apply unchanged" is not true

- **Section:** Host-call specification (encodings); Determinism and consensus; Test vectors
- **Severity:** major
- **Comment:** The determinism section relies on Ethereum's vectors to "pin these behaviours", but the draft departs from Ethereum's semantics exactly where clients tend to diverge:
  - §1 returns `HUH` for "a scalar not below the group order". EIP-196's ECMUL takes any 256-bit scalar, and EIP-2537 states explicitly that MSM scalars need not be below the subgroup order.
  - There is no BN254 MSM precompile. EIP-196 defines ADD and MUL only.
  - §2 requires k ≥ 1. EIP-197 accepts empty input and returns 1.
  - §6 writes 20 bytes and returns `HUH` on failure. The 0x01 precompile returns a 32-byte left-padded word, or empty output. The draft does not say that v's upper 31 bytes must be zero. Its failure list in parentheses reads as exhaustive but omits r not being an x-coordinate on the curve.
  - §5 returns φ'8 = 0 on a versioned-hash mismatch. EIP-4844's precompile fails, and returns 64 bytes on success.

  Each choice can be defended. But each one means the vectors do not apply unchanged, and nothing else pins the behaviour.
- **Suggested change:** For each call, either adopt the EIP semantics verbatim or write a normative mapping table (EIP outcome → φ'7, φ'8, memory). Ship JAM-native vector files in the JIP, inputs with expected registers, memory and gas, covering every `HUH` branch.

### GW-6: `kzg_point_evaluation` is redundant, mispriced and Ethereum-specific

- **Section:** §5 `kzg_point_evaluation`; Gas pricing table; Reference implementations
- **Severity:** major
- **Comment:** A KZG opening is one 2-pair BLS12-381 pairing check over a 3-point G1 MSM: e(C − [y]G1 + [z]π, −G2) · e(π, [s]G2) = 1. With the draft's own constants, that is 15,590,400 for call 104 plus about 4.4M for a 3-point call 103 (168 + 2 × 53.6 µs interpolated, × 16), about **20M plus a SHA-256 in PVM code**. G_KZG is **67,888,000**, 3.4× more for the dedicated call. The reason is that G_KZG was timed on zkcrypto, the same pure-Rust code as the PVM baseline, while the draft names c-kzg-4844 (blst) as the native implementation. Beyond price, the call bakes into JAM's consensus Ethereum's versioned-hash format (0x01 ‖ sha256(C)[1..]) and Ethereum's ceremony output, and it does so by reference ("the same [s]G2 as EIP-4844"), not by value.
- **Suggested change:** Drop call 105. Publish the composition (calls 103 and 104 with the versioned-hash check in the service) as a library, and keep the EIP-4844 vectors as its test. If it is kept, price it from the implementation named, state [s]G2's bytes normatively in the text, and justify why an Ethereum data format belongs in the GP.

### GW-7: The Poseidon2 justification is contradicted by the project's own architecture

- **Section:** §7 `poseidon2_bn254_permute`; Open question 2
- **Severity:** major
- **Comment:** §7 says that "without it, every tree update must be proven and applied in order, one package a slot". `docs/privacy/ARCHITECTURES.md` recommends candidate 3, which keeps all hashing in one refine a slot and reaches 145–2,900 transfers a slot a tree with no host call. There, JIP-8's Poseidon2 only "doubles" the insertion rate. In accumulate, the draft's 8.3M for one depth-40 insertion buys one note per report, "with little room to spare", before the 40 frontier reads and writes are counted (`read` 2407 and `write` 2442 base gas each, `definitions.tex:331,337`). Poseidon2's f is 4.7, the lowest measured, and its saving 2.1× the smallest of the seven. Meanwhile the call fixes one project's round constants (Noir and Barretenberg's, "pinned by their published vectors") into the GP, and Open question 2 already foresees Poseidon over BN254 and Rescue queuing behind it.
- **Suggested change:** Remove call 107 from this JIP. If it returns, it should come as a separate proposal, motivated by refine throughput with candidate 3 measured, its constants stated normatively in the text, and a policy for which hashes qualify.

### GW-8: Activation is undefined for refine and is-authorized

- **Section:** Determinism and consensus; Compatibility
- **Severity:** major
- **Comment:** "Adopted at an activation slot" works for accumulate, which keys off the block's timeslot. Refine does not run at one slot. Guarantors run it, and auditors re-run it up to about two epochs later, which is exactly why the GP binds refine's view of state to the package's lookup-anchor slot (`pvm_invocations.tex:63`, `:76`, `:82–98`). Is-authorized takes only (p, c) (`pvm_invocations.tex:37`) and has no slot at all. If activation is read from the executing node's current slot, a package refined just before activation and audited just after gets `WHAT` from one execution and `OK` from the other, with different gas, and that is a dispute. The draft also never says how a service detects support. Before activation a probe costs Cgasunknown = 1000 (`definitions.tex:306`, `pvm_invocations.tex:52–53`) and returns `WHAT`. And there is no versioning story for the next curve.
- **Suggested change:** Key activation for Ψ_R and Ψ_I to the work-package context (the lookup-anchor timeslot), and say so in the dispatch functions. Specify detection: a cheap probe whose outcome is defined both before and after activation. Reserve the index range and state how later additions are allocated.

### GW-9: Client diversity and the shared-library argument

- **Section:** Motivation (last paragraph); Determinism and consensus; Reference implementations
- **Severity:** major
- **Comment:** "Implemented in widely used, audited libraries (blst, arkworks, k256), which every client already depends on or can" turns the problem upside down. JAM's assurance comes from many independently written clients. The GP needs BLS12-381 (`beefy.tex:4`), Ed25519, Bandersnatch, Keccak and Blake2b. It does not need BN254 or secp256k1, so for most teams those are new dependencies in consensus code. There are two outcomes. If everyone links the same library, client diversity buys nothing on these calls and one library bug becomes a network-wide fact. If teams write their own, as clean-room implementations in languages without mature pairing libraries, they diverge on edge cases. The repository already has evidence of how hard exact agreement is: a mixed lasair/PolkaJam net forked over 2 gas (`docs/privacy/LIMITS.md`, row 6). Whether third-party crypto libraries are acceptable under the clean-room expectations for JAM implementations is a question the draft should answer, not assume.
- **Suggested change:** Make the specification normative in the GP's own terms (curve parameters, encodings, canonicality, subgroup rules, infinity), independent of any library. Commit to conformance vectors and a differential fuzzing harness across at least two independent implementations per call before activation. Say plainly which curves are new dependencies for a GP-only client.

### GW-10: The headline use case is not measured

- **Section:** Motivation; Gas pricing (constants table, "no PVM baseline")
- **Severity:** major
- **Comment:** The draft is motivated first by proof verification (UltraHonk, 170–181M gas), yet the four calls that would serve it, 101–104, have no PVM baseline. Nothing estimates how much of the verifier they would remove. `spikes/epoch-proof/RESULTS.md` shows the verifier gaining about 1.3M gas per doubling of the circuit from sumcheck and Gemini, field arithmetic and hashing that no proposed call touches. Without a profile, Amdahl's law could leave the saving well below 2×. The motivation also leads with "about 50 times", a share-of-budget comparison that the pricing section itself calls misleading, and that ignores JAM's 341 cores (`docs/privacy/LESSONS.md` §3a).
- **Suggested change:** Measure the UltraHonk verifier with its pairing and MSM replaced by stubs, or profile its gas by function, and report the projected gas with calls 101–102. Lead the motivation with that number and the recoverable 2–4×, and move the Ethereum comparison to context.

### GW-11: Scope, and how a GP change is proposed

- **Section:** Whole draft; Open questions 2–4; Status line
- **Severity:** major
- **Comment:** Host calls cannot be withdrawn once services depend on them. Seven calls over three curves, a hash and a setup, with Grumpkin, the G2 MSM and more hashes in Open questions 2–3, is a precompile catalogue, and that is what JAM set out not to have. The draft also says it is "a GP amendment", but every merged JIP standardises something beside the GP and none changes a GP rule (`docs/jip/FORMAT.md`). The draft does not say where the normative text lands or who ratifies it.
- **Suggested change:** Split the proposal. The core JIP is BLS12-381 and BN254 pairing check plus G1 MSM: BLS12-381 is already in the GP, and BN254 is what deployed proof systems target. secp256k1, KZG and Poseidon2 would each be separate, each with its own measured case. Add a "Process" paragraph: the JIP as rationale, a concrete diff against the GP's Appendix B and `definitions.tex`, and the body that decides. Answer Open question 4 here: separate indices, which keep gas and conformance per call simple.

### GW-12: `secp256k1_recover` returns an Ethereum address, not a key

- **Section:** §6 `secp256k1_recover`
- **Severity:** minor
- **Comment:** Deriving keccak256(pk)[12..] inside the call makes it Ethereum-specific. Bitcoin and Cosmos derive addresses differently, and a light client may want the key itself. Its only stated use is Aztec's attestations (`spikes/ecdsa-gas`).
- **Suggested change:** Return the 64-byte uncompressed public key, and leave address derivation to the service. If Ethereum compatibility matters, note that the service hashes with Keccak, which the GP already defines.

### GW-13: Is-authorized availability is unmotivated, and in one case impossible

- **Section:** Side-effects ("Every call is available in refine, accumulate and is-authorized")
- **Severity:** minor
- **Comment:** Ψ_I is "totally stateless" and dispatches only `gas`, `grow_heap` and `fetch` (`pvm_invocations.tex:33–50`). Adding seven calls changes that design, and the draft gives no use case. G_I is 50M (`definitions.tex:266`). G_KZG, at 67.9M, exceeds the whole budget, and a 2-pair BN254 check (20.5M) takes 41% of it.
- **Suggested change:** Exclude Ψ_I unless a measured authorizer case is given, such as a BLS aggregate-signature authorizer. If one is, add the calls explicitly to F in eq. `isauthorizedmutator`.

### GW-14: Factual corrections

- **Section:** Motivation; Gas pricing; Compatibility
- **Severity:** minor
- **Comment:**
  - The host-call list omits `grow_heap` (1) and `historical_lookup` (7). GP 0.8.0 has 28 calls, 0–27 (`pvm_invocations.tex:311`, `:547`; the same omission is in `LIMITS.md`). "0–27" in Compatibility is correct.
  - "The GP's reference hardware" and "reference machine" do not exist (see GW-2).
  - "Return `WHAT` as today" should add the 1000 gas.
  - `spikes/jip-bench/RESULTS.md` says that the KZG row's gas "differs from the local 199,453,527", but both its tables print 199,453,527.
  - What the draft gets right, briefly: charging in full before execution from the length alone matches the GP's block-level pre-charging (`pvm.tex:158`). Panicking on inaccessible memory matches the GP's convention (e.g. `pvm_invocations.tex:409`). Using φ'8 for a second result has precedent (`invoke`, `query`). Recognising that this cannot be a JIP-1-style optional extension is correct.
- **Suggested change:** Correct the list and the wording, and fix the RESULTS sentence.
