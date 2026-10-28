# aztec-on-jam

**Aztec on JAM: private payments, proved on your device, verified by the chain.**

[Aztec](https://aztec.network) is a privacy network — every transaction is a zero-knowledge
proof made on the sender's device, so the chain verifies it without learning sender, receiver
or amount. [JAM](https://graypaper.com) is Polkadot's next protocol — hundreds of cores of
verifiable compute. This project puts JAM in Ethereum's place under Aztec: Aztec's software as
released, our code only at the seam.

**Docs: [abutlabs.github.io/aztec-on-jam](https://abutlabs.github.io/aztec-on-jam/)**
— the [architecture](https://abutlabs.github.io/aztec-on-jam/aztec/), the
[demo](https://abutlabs.github.io/aztec-on-jam/demo/), and
[our proposed JAM host calls](https://abutlabs.github.io/aztec-on-jam/jip/).

![Alice makes a token and mints privately; Bob opens his wallet; Alice sends Bob 40, proved in her browser; JAM's validators verify and finalize it; Bob finds his 40; Aztec's prover proves the epoch and JAM verifies that proof — each step shown live beside the wallet](docs/images/demo.gif)

Recorded live: Aztec's released software on six JAM validators, real proofs throughout. Waits
are sped up, never cut.

## Quickstart

Aztec's own stack — wallet, sequencer and prover, as released — settling on six JAM validators.
Needs Rust, Python 3.11+, Node 22+, Chrome and Docker; `bootstrap.sh` installs the rest at pinned
versions ([docs/TOOLS.md](docs/TOOLS.md)).

```sh
git clone https://github.com/abutlabs/aztec-on-jam && cd aztec-on-jam
scripts/bootstrap.sh                                  # toolchain, Aztec's packages, the wallet
DEVNET_NET=jb-lasair6 tools/aztec-local/on-jam.sh     # everything up; prints each step and the wallet's URL
```

`DEVNET_NET=jb-lasair6` runs six **lasair** validators with the JIP's native curve math (needs the
lasair checkout beside this one); leave it out for PolkaJam's testnet. Ctrl-C stops everything;
`tools/aztec-local/on-jam.sh down` cleans up after a run in another terminal. In another
terminal, `node tools/aztec-local/send-tx.mjs` makes a private transfer end to end.

What happens in one payment, and how to check it: [docs/DEMO.md](docs/DEMO.md).

## Status

| Milestone | |
| --- | --- |
| Aztec v6's own node, unchanged, settling on JAM through our sidecar, real proofs | **done** · [architecture](https://abutlabs.github.io/aztec-on-jam/aztec/) |
| Our wallet on Aztec's libraries, with a live view of who does what — the recording above | **done** · [demo](https://abutlabs.github.io/aztec-on-jam/demo/) |
| Aztec's prover on a laptop; every epoch proof verified and finalized by JAM | **done** · [run it](https://abutlabs.github.io/aztec-on-jam/operators/) |
| Aztec's transaction proofs verified on a live JAM net by Aztec's own verifier | **done** — needs our proposed host call: 3.46B gas with it, 11.4B (over two cores' budget) without · [proposal](https://abutlabs.github.io/aztec-on-jam/jip/) |
| A committee of several nodes, and every client proof checked in refine | **in progress** · [#71](https://github.com/abutlabs/aztec-on-jam/issues/71) |

## The proposal

Verifying a zero-knowledge proof is mostly elliptic-curve arithmetic, which JAM's VM meters as
ordinary code. We drafted a JAM Improvement Proposal adding five curve host calls — the
operations Ethereum ships as precompiles. Measured on a live net: verification 3 to 16×
cheaper, and Aztec's own client proofs become verifiable at all.
[The case and the numbers](https://abutlabs.github.io/aztec-on-jam/jip/) · [full
text](docs/jip/jip-final.md).

## Layout

| Path | What |
| --- | --- |
| `services/aztec-settlement/` | Aztec's Rollup, Inbox and Outbox as a JAM service: refine verifies, accumulate records |
| `tools/aztec-local/` | Aztec v6's released node and prover on JAM: the sidecar (`jam-l1.mjs`) and `on-jam.sh` |
| `wallet/` | our wallet on Aztec's libraries: the PXE and Aztec's prover in the browser tab; the films |
| `crates/` | the curve calls, KZG and Aztec's L1 encodings, shared by the services |
| `services/privacy/`, `circuits/`, `client/` | the proposal's benchmark: an UltraHonk verifier at Aztec's sizes, its circuit and prover |
| `nets/` | the JAM testnets: PolkaJam's, six lasair validators, mixed |
| `spikes/` | the measurement behind every number, each with its script |
| `docs/` | [DEMO.md](docs/DEMO.md) (the implementer's tour), [aztec/STACK.md](docs/aztec/STACK.md) (the port, mapped), [jip/](docs/jip/jip-final.md) (the proposal), [LEDGER.md](docs/LEDGER.md) (every step, with its evidence) |

Apache-2.0. An independent project — not affiliated with Aztec Labs or the Polkadot Fellowship.
