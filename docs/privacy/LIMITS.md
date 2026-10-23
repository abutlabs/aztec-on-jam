# Fundamental spec limits for privacy on JAM, and whether a JIP is warranted (#31)

Aodh's question (#35): *if this exercise has revealed or underscored a fundamental challenge for
current JAM specs to facilitate Aztec-like privacy (reminiscent of how the original zk.money
was shuttered as a dapp on Ethereum L1 due to unreconcilable challenges), then formulate and
propose a JIP.*

**Decision: yes, a JIP is warranted, for one limit: the PVM has no cryptographic host
functions.** It is spec-level (Gray Paper 0.8.0), it is measured, it costs every
privacy system on JAM about 50 times what Ethereum charges, in share of the budget they run in,
for checks Ethereum prices as precompiles (about 3 times for a whole proof verifier, #40), and it can be eased without touching JAM's model. Not removed: a host call saves the
recompiled PVM's overhead on this code (f = 4.7–9.4, measured on x86, #42) divided by a safety
margin: about 2–4× at the draft's m = 2 (`spikes/jip-bench`), not
the whole gap, much of which is Ethereum pricing precompiles low against its slow EVM. Even 2×
matters: it brings a depth-40 Poseidon2 tree insertion inside accumulate's 10M gas. The other limits found
are either workable with service design, not spec-level, or belong to clients (below). #32
drafts the JIP.

## The candidates

| # | Candidate | Spec-level? | Evidence | Verdict |
|---|---|---|---|---|
| 1 | **No cryptographic host functions** | **Yes.** The host calls (GP 0.8.0 `pvm_invocations.tex`) are gas, fetch, lookup, read, write, info, export, machine, peek, poke, pages, invoke, expunge, bless, assign, designate, checkpoint, new, upgrade, transfer, eject, query, solicit, forget, yield and provide: none hashes, adds curve points or checks a pairing | Measured on PolkaJam: KZG point evaluation 199,453,527 gas (#24), ecrecover 13,119,449 (#39), an UltraHonk check 170–180M (#23). Against Ethereum's precompiles (50,000 and 3,000) at a 60M block: about 48× and 52× the share of the budget the check runs in, and about 3× for a whole UltraHonk verifier, which on Ethereum is contract code using curve precompiles (`LESSONS.md` 3a, #40) | **The JIP's subject** |
| 2 | Refine's per-package gas against proof systems built for cheap proving | Partly: W_G = 5e9 is a constant | Aztec's own kernel proof (CHONK) would need about 12B gas, 2.4 packages (spikes/chonk). Its epoch proof needs about 180M (#23) | Not a limit for Aztec's architecture, which folds before settlement. Curve host functions would likely cut CHONK's cost too (its verifier spends its time in an IPA over Grumpkin, spikes/chonk), by reasoning, not measured. Covered by 1 |
| 3 | One shared private state takes one package a slot | No: it follows from proofs naming the exact current root, which is a service's choice | The soak: one tree, one package a slot, 600 of 600 (#4). If accumulate could insert notes itself, proofs could name a recent root (Zcash's anchors), and many packages could share a tree each slot. That needs cheap hashing in accumulate (10M gas a report) | Measured (#41): one Poseidon2 hash costs 428,841 gas in the PVM, and a depth-40 path 17.1M, so accumulate's 10M pays for 0.58 of one insertion. But the insertions can move to refine: one tree package a slot appends 11,350 notes as a batch (#43, `ARCHITECTURES.md` candidate 3), so this is **workable under today's specs**. Poseidon2 stays in the JIP because it doubles that rate and lets accumulate insert a note itself (8.3M a depth-40 path) |
| 4 | Data that must outlive auditing | No | Exported segments are kept 28 days (`assurance.tex:7`), past Ethereum's 18.2-day blob window (#24) | Not a limit: JAM does better than Ethereum |
| 5 | A 200 KiB work-package message | No: JAM-NP, not the Gray Paper | Payloads over 204,533 bytes are refused (#36); extrinsics carry bulk data to W_B = 13.8 MB (#37) | A networking detail, documented |
| 6 | Exact gas agreement across clients for heavy code | Spec-level in that gas is the GP's, but a client bug | A mixed net forked over 2 gas (ledger 23) | Heavy cryptography in the PVM makes it likelier; host functions with fixed gas reduce it. Covered by 1 |

## zk.money's lesson, as the sources state it

Aodh's framing is that zk.money was shut down on Ethereum L1 over unreconcilable challenges.
Aztec's own account is narrower. Aztec announced the sunset of Aztec Connect, which ran
zk.money, on 13 March 2023, and disabled deposits on 31 March 2023. It stopped producing
blocks on 31 March 2024, open-sourced the code and invited forks. It gave its reason as
focusing on a general-purpose encrypted chain: the network now in alpha
([Aztec's post](https://medium.com/@aztecnetwork/sunsetting-aztec-connect-a786edce5cae);
[Cointelegraph](https://cointelegraph.com/news/privacy-focused-blockchain-network-closes-aztec-connect-tool)).
Neither source names L1 cost or a technical wall as the reason, so this file does not claim
one.

The parallel that the evidence supports is narrower. Aztec Connect was an application-shaped
privacy system (fixed bridges, a fixed transfer circuit) on a base layer that offered it only
generic execution and expensive data. Aztec moved to a general-purpose rollup, still settling on Ethereum, whose kernel
circuits, trees and proving it controls. On JAM, a privacy service can be its own chain's worth of
logic (#28), but every proof it checks pays the interpreted price of cryptography. That is
the one thing a service cannot fix for itself, and so the one a JIP should.

## What the JIP should and should not do

- **Do:** add host functions for the primitives privacy systems verify. Pairing checks and
  multi-scalar multiplication on BN254 and BLS12-381, KZG point evaluation, and secp256k1
  recovery, with gas priced from benchmarks and deterministic results every client must
  match. Add Poseidon2 over BN254, which #41 shows accumulate needs: a depth-40 insertion costs 17.1M
  gas in PVM code, against accumulate's 10M a report.
- **Do not:** add privacy-specific protocol features (shielded pools, note trees) to the
  protocol. JAM's generality is a strength, and the evidence shows services can build these
  (this repository and #28).
- **Keep the PVM's guarantees:** host functions must be deterministic, metered before
  execution, and cheap for every client to implement, from widely used libraries (blst,
  arkworks, k256).
