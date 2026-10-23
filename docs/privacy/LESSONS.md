# What this exercise taught about privacy on JAM (#29)

Aodh's question (#35): *what did this exercise teach us about JAM's ability to provide privacy
at a scale and quality that could rival Aztec's vision and standards under JAM's current
specs?*

The short answer: **JAM can check private computation as well as any chain can, and JAM's
re-execution gives a stronger guarantee than a proof alone. What it lacks is cheap
cryptography and a way to share one private state across many packages a slot.** The first
makes a single cryptographic check (a KZG opening, a signature) cost about 50 times more of the
budget it runs in (one core's package) than the same check costs of an Ethereum block, and a
whole proof verifier about 3 times more, though JAM's many cores still give it more capacity in
total. The second
limits one privacy pool to one package a slot unless proofs are folded off the chain, which
brings Aztec's prover back. Neither is a wall: both have measured workarounds on PolkaJam. Both
are spec-level costs a protocol change could remove (#31, #32).

Every number below was measured in this repository, most of them on a six-validator PolkaJam
v0.8.0 testnet with PolkaJam's own telemetry. Each one names its source.

## 1. What works today, measured

| Claim | Evidence |
|---|---|
| A user's zero-knowledge proof, made in a browser, is checked by every validator that audits the package | UltraHonk verified in refine, 169,112,761 gas (`services/privacy/check.sh`); a browser proves in 1.0 s (README) |
| Private state is kept correctly and cheaply | Accumulate writes a transfer for 103,089 gas, 1% of a report's 10M (README); a replayed or stale proof changes nothing (`nets/demo.sh check`, 37 assertions) |
| It runs for an hour without losing anything | 600 of 600 packages, one a slot, median 13.4 s from submission to state, finality never stalled (#4, ledger 28) |
| Aztec's own proofs can be checked: its epoch (root rollup) proof | Aztec's proof shape (plain UltraHonk, Keccak, 113 public inputs) verified in refine; with every gate type in use, 170,364,998 gas at 2^16 to 180,791,299 at Aztec's 2^24, proof of 331 fields as Aztec's (#23) |
| Aztec's data can be kept: its blobs | EIP-4844 point evaluation 199,473,075 gas; one blob against its commitment 262M; an Aztec checkpoint's 6 blobs batch-checked in one tiny package (967,993,405); exported as segments, kept 28 days, re-read and re-checked by a later package (#24, #37) |
| Aztec's software needs few changes | The PXE runs unchanged; the node's Ethereum code (about 11k lines) sits mostly behind interfaces; 4 seams would make a JAM backend a plug-in (#25); one Aztec release moved that boundary little (#27) |

## 2. Quality: where JAM is stronger than Aztec's model

- **Re-execution, not only a proof.** On Aztec (and any proof-only rollup) the proving system
  is the security: a bug in it is a bug in every block. Aztec had one, in July 2026 (the
  tutor's *JAM beside Aztec*). On JAM, refine is re-run by guarantors and audited, so a
  service's verifier bug is that service's bug, and the auditors still check that the code ran
  as written.
- **Data stays longer.** JAM keeps exported segments 28 days (GP `assurance.tex:7`), past
  Ethereum's 18.2-day blob window (#24, `docs/aztec/BLOBS.md`).
- **One privacy set for every service.** Any JAM service can pay into a privacy service by a
  deferred transfer, with no bridge (`docs/DEMO.md`).

## 3. Scale: where JAM strains

**a. Cryptography is interpreted.** The PVM has no host functions for pairings or elliptic-curve
arithmetic. Every check is metered PVM code, where Ethereum prices the same checks as
precompiles. Gas units differ between the chains, so this compares shares of the budget each
check runs in: one core's full package (5e9 refine gas) on JAM, one block on Ethereum (60M,
the mainnet limit since Fusaka's EIP-7935):

| Check | On JAM (refine, PolkaJam) | Share of a full package | On Ethereum | Share of a block (60M) | JAM's share over Ethereum's |
|---|---|---|---|---|---|
| One KZG point evaluation | 199,473,075 (#24) | 4.0% | 50,000 (EIP-4844 precompile) | 0.083% | about 48× |
| One ECDSA recovery | 13,119,449 (#39) | 0.26% | 3,000 (precompile 0x01) | 0.005% | about 52× |
| 33 ECDSA recoveries (an epoch's attestations) | 432,756,815 (#39) | 8.7% | 99,000 | 0.17% | about 52× |
| One UltraHonk verification (Aztec's epoch proof, 2^24) | 180,791,299 (#23) | 3.6% | 721,239 (bb's optimized Solidity verifier, as Aztec builds it, #40) | 1.2% | about 3× |

The UltraHonk row is narrower because Ethereum's verifier is contract code too, using
precompiles only for its curve operations (`spikes/evm-verifier`).

**But per chain, JAM has more room.** JAM has 341 cores, each with a full package every 6
s; Ethereum has one 60M block every 12 s. Taking the point evaluation alone: JAM could run
341 × 25 = 8,525 every 6 s (about 1,400 a second) against Ethereum's 1,200 every 12 s (100 a
second), about 14 times more in total. **Each single check is about 50 times dearer in the
unit a service buys (a whole proof verifier about 3 times), and the chain as a whole can do
more of them.** For a privacy service, the unit is what matters: it pays for its own core's
package.

**b. One shared private state takes one package a slot.** Each proof names the state it
started from, and refine cannot read state, so packages on one tree must land in order, one a
slot: one transfer every 6 s per tree unfolded (#4, the soak; `docs/DEMO.md`). Scale needs
either folding many transfers into one proof off the chain (Aztec's prover, returned) or many
trees (more cores, smaller crowds). Aztec on JAM inherits Aztec's folding, so its throughput is
Aztec's. JAM then checks each checkpoint's blobs (about 968M gas for 6, batched) and one epoch
proof with its attestations (about 812M) (#23, #37, #39).

**c. Bulk data must travel as extrinsics.** JAM-NP caps the work-package message, payloads
included, at 200 KiB (#36). Blobs go as extrinsics, bounded by the GP's 13.8 MB bundle. That is
workable, but it is a protocol detail a privacy builder must know.

**d. Clients must agree on gas to the unit.** A mixed lasair and PolkaJam net forked over 2
gas on one accumulate (ledger 23). Heavy cryptographic code in the PVM puts the most gas at
stake.

**e. The prover follows a moving target.** Barretenberg changed its proof format at every
major release (bb 3 → 5 → 6); each port of the refine verifier was small, but never zero (#15,
#22, #27).

**f. The client's work grows with the chain.** Our prover rebuilt its trees from every note,
and an hour's soak slowed to a quarter until it was fixed. A long-lived chain needs an indexer
that serves witnesses, as Aztec's node does for its PXE (ledger 28).

## 4. So, could JAM rival Aztec's standards?

- **On quality, yes and more:** the same zero-knowledge proofs and client-side privacy, with
  re-execution and auditing on top, and longer data availability.
- **On scale, not by itself under today's specs.** A JAM-native privacy service without
  folding is one transfer every 6 s a tree. With Aztec's folding it can match Aztec's
  throughput. Settlement then costs two packages for each Aztec checkpoint and epoch: about
  981M gas a checkpoint (blobs and the proposer) and about 812M an epoch proof (proof, blob
  opening, attestations). Each fits a tiny package (#23, #24, #39), at the price of Aztec's
  prover tier off the chain.
- **The gap is cost, not possibility.** Cheap pairings, curve operations and hashes as host
  functions, and a way for many packages in a slot to update one private state, would close
  most of it. Whether that warrants a protocol change is #31's question; what the change would
  be is #32's.

## Sources outside this repository

- Ethereum's block gas limit, 60M on mainnet since Fusaka (EIP-7935); Glamsterdam's move toward
  200M was on testnets as of October 2026:
  [The Block](https://www.theblock.co/post/380687/ethereum-block-gas-limit-fusaka),
  [ethereum.org, *Building on Ethereum in 2026*](https://ethereum.org/latest/building-on-ethereum-in-2026/),
  [CoinDesk, 8 October 2026](https://www.coindesk.com/tech/2026/10/08/ethereum-s-glamsterdam-test-runs-near-200-million-gas-per-block-after-upgrade).
- Precompile prices: EIP-4844's `POINT_EVALUATION_PRECOMPILE_GAS = 50000`; ecrecover (0x01)
  3,000 gas (the Yellow Paper, Appendix E).
- Aztec's V5 proving-system vulnerability (July 2026): the tutor's *JAM beside Aztec*, citing
  Aztec's own post.
