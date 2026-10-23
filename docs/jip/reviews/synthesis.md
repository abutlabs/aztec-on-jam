# JIP-8 reviews: synthesis (#33)

*The two reviews and their cross-responses are simulated: written by an AI in the personas of
Gavin Wood (GW) and Zac Williamson (ZW) for internal QA. They are not those people's views and
neither has seen them.*

The reviewed text is `docs/jip/jip-draft.md` at 4f0fd19. The files:
- `gavin-wood.md`: GW-1 to GW-14.
- `zac-williamson.md`: ZW-1 to ZW-14.
- `gavin-wood-responds-to-zac-williamson.md` and `zac-williamson-responds-to-gavin-wood.md`.

Each comment's row below gives its author's severity, the other reviewer's position, and what
the final JIP (#34) does. Where both reviewers agree, the final adopts the change. Where they
disagree, this file chooses and says why.

## The reviewers agree on

1. **Narrow the JIP to a core:** BN254 and BLS12-381 pairing checks and G1 MSMs, at separate
   indices (GW-11, ZW-13, both "Where we agree"). secp256k1, Poseidon2 and Grumpkin each go to
   a later proposal with its own measured case.
2. **Drop `kzg_point_evaluation`.** The check composes from the MSM and pairing calls for about
   20.0M gas, against 67.9M as priced (GW-6, ZW-5, both checked the arithmetic).
3. **State the pricing invariant and anchor it in the GP's unit,** not in one recompiler's
   wall-clock speed (GW-2, ZW-11). ZW's response shows the anchor moves the saving from 2–4× to
   5–19×.
4. **Re-benchmark end to end before any constant is proposed:** from encoded bytes, with every
   decoding, on-curve and subgroup check the specification requires, on random and adversarial
   inputs, with high percentiles, and R as a slope (GW-3, ZW-10; both call it blocking for the
   constants).
5. **One normative gas schedule per call:** a tabulated MSM schedule, φ8 capped, and a fixed
   order of evaluation (GW-4, ZW-3).
6. **Exact semantics and JAM-native vectors** in place of "Ethereum's vectors apply unchanged"
   (GW-5, ZW-9).
7. **Measure the gain where it matters,** UltraHonk with the curve operations metered, and lead
   the motivation with it, not with the 50× share comparison (GW-10, ZW-1, ZW-2).
8. **A Rationale:** primitives, not verifiers; whole operations, not field-arithmetic host calls
   (ZW-13, GW's response to it).
9. **Activation keyed to the work-package context,** a support probe, and no is-authorized
   access without a use case (GW-8, GW-13).
10. **The factual slips:** the host-call list, "reference machine", `WHAT`'s 1000 gas, stale
    citations (GW-14, ZW-14).

## Where they disagree, and the choice

| Question | GW | ZW | Final JIP |
|---|---|---|---|
| Must an ISA or gas-table change be shown to fail first? (GW-1) | Yes: blocking | No: complementary, since pairings and MSMs gain from algorithms (precomputed lines, endomorphisms, Pippenger) that a better multiply can't bring | **Complementary.** ZW's point holds on the evidence: the PVM's f is only 4.7–9.4 on this code, and the call's gain is largely the library's. The final JIP adds an "Alternatives considered" section, and the ISA experiment is filed as #47 |
| A fixed-G2 BN254 pairing call (ZW-4) | Disagree: it puts Aztec's setup into consensus | Proposes it | **Not adopted.** It names an external setup in the protocol, the reason both reviewers drop KZG. The end-to-end benchmark (#45) measures the per-pair G2 cost. Caching is a client's own optimisation and does not change gas |
| Add `grumpkin_msm` now (ZW-12) | Disagree: its only consumer awaits a re-estimate | Proposes it | **Not now.** Q3 is answered in the text: Aztec settlement needs no Grumpkin; a direct CHONK check does. A follow-on JIP waits for #46's amortised CHONK estimate |
| Poseidon2: remove or keep (GW-7, ZW-8) | Remove from this JIP | Accepts a separate JIP, in parallel, if the motivation is restated | **Removed from JIP-8.** The #43 measurement settles it: a refine-side tree takes 11,350 notes a package, so insertion is not the bound. Its live case is bb's Poseidon2-transcript verifiers, to be measured for a follow-on |
| A whole-operation blob batch call (ZW-2 option 1) | Disagree: Ethereum's blob format in consensus | Offers it as one option, not a demand | **Not adopted.** Blob evaluation stays in the PVM. The final JIP says so, citing BLOBS.md's alternatives |
| secp256k1: address or public key (GW-12) | Return the public key | Agrees; services must still enforce low s (ZW-6) | Moves out with the call; recorded for its follow-on |
| EIP semantics or HUH (GW-5, ZW-9) | Adopt the EIPs' semantics outright | Either | **Adopt the EIPs':** scalars reduced mod r, k = 0 returns 1. Fewer divergences, and a verifier never notices |

## Every comment

| Comment | Severity | Other reviewer | Final JIP (#34) |
|---|---|---|---|
| GW-1 ISA route first | blocking | ZW: in part | Alternatives section; #47 |
| GW-2 pricing anchor | blocking | ZW: in part (agrees on the anchor, shows its effect) | Invariant stated in the GP's unit; savings recomputed |
| GW-3 benchmark omits checks; R from totals | major | ZW: agree (its fixed-cost subtraction is not valid across services) | #45, then constants |
| GW-4 gas schedule ambiguous, overflow | major | ZW: agree | Tabulated schedule, φ8 cap, evaluation order |
| GW-5 vectors don't apply unchanged | major | ZW: agree | EIP semantics adopted, mapping and vectors |
| GW-6 KZG call redundant | major | ZW: agree | Call removed, composition in Suggestions |
| GW-7 Poseidon2 justification | major | ZW: in part | Call removed (above) |
| GW-8 activation for refine and Ψ_I | major | ZW: agree | Keyed to the lookup-anchor slot; probe |
| GW-9 client diversity, shared libraries | major | ZW: in part (two mature implementations fuzzed, not clean-room) | Normative text plus differential fuzzing of two or more implementations |
| GW-10 headline case unmeasured | major | ZW: in part (Amdahl concern not borne out) | #44 measures it |
| GW-11 scope, process | major | ZW: in part (Grumpkin later, not never) | Core of four calls; Process paragraph |
| GW-12 secp256k1 returns an address | minor | ZW: agree | Out with the call |
| GW-13 is-authorized availability | minor | ZW: agree | Ψ_I excluded |
| GW-14 factual corrections | minor | ZW: agree | Fixed (the RESULTS sentence already is) |
| ZW-1 UltraHonk gain unestimated | major | GW: agree | #44 |
| ZW-2 settlement gain unstated | major | GW: in part (no blob call) | "With JIP-8" column, estimated; blob evaluation stays in the PVM |
| ZW-3 MSM pricing shape | major | GW: in part (3.3's inference compares unlike things) | Tabulated schedule; G(1) checked against a standalone PVM scalar multiplication (#45) |
| ZW-4 fixed-G2 pairing | minor | GW: disagree | Not adopted (above) |
| ZW-5 KZG mispriced | major | GW: agree | Call removed |
| ZW-6 high s against OpenZeppelin | minor | GW: agree | Out with the call; noted in `docs/aztec/SERVICE.md` |
| ZW-7 Poseidon2 spec too loose | major | GW: agree | Out with the call; the checklist recorded for the follow-on |
| ZW-8 Poseidon2 motivation | major | GW: in part | Call removed (above) |
| ZW-9 EIP divergences | minor | GW: agree | EIP semantics adopted |
| ZW-10 benchmark not end to end | blocking | GW: agree | #45 |
| ZW-11 BN254 priced above BLS12-381 | minor | GW: agree | Priced from a stated reference; a second BN254 library in #45 |
| ZW-12 Grumpkin, Q3 | major | GW: in part | Q3 answered; Grumpkin later; #46 |
| ZW-13 Rationale, Q4 | minor | GW: in part (ISA instructions are not field-arithmetic calls) | Rationale; separate indices |
| ZW-14 stale citations | minor | GW: agree | Fixed in `docs/aztec/SERVICE.md` and kzg-gas's RESULTS |

## Facts the reviews checked

- **KZG composed:** 4.40M (a 3-point BLS12-381 MSM) plus 15,590,400 (a 2-pair check), about
  20.0M, against G_KZG's 67,888,000.
- **The GP's sizing:** 5e9 gas over 1.5–3 s gives 1.67–3.33 gas/ns (`discussion.tex:77`), and
  accumulate gives 1 gas/ns (`:79`).
- **Marginal secp256k1 R:** 7.77 gas/ns, from (432,756,815 − 13,119,449) / (62.379 − 8.374) ms.
- **Sumcheck share:** about 5% of an UltraHonk verification. A rejected 2^24 proof stops after
  sumcheck at 9,831,418 gas, against 180,791,299 for an accepted one.
- **Poseidon2's parameters** match Barretenberg's exactly, against bb's own test vector.
- **Aztec's root rollup** fully verifies its IPA (`proof_data.nr:64-66`), so the epoch proof is
  BN254 KZG only.

The reviews' aztec-packages paths are at commit 0219f11, which is tagged both v6.0.0 and
v6.0.0-nightly.20261021.
