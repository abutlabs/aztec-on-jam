# jam-privacy-service

**Aztec on JAM: private payments, proved on your device, verified by the chain.**

[Aztec](https://aztec.network) is a privacy network — every transaction is a zero-knowledge
proof made on the sender's device, so the chain verifies it without learning sender, receiver
or amount. [JAM](https://graypaper.com) is Polkadot's next protocol — hundreds of cores of
verifiable compute. This project puts JAM in Ethereum's place under Aztec: Aztec's software as
released, our code only at the seam.

**Docs: [abutlabs.github.io/jam-privacy-service](https://abutlabs.github.io/jam-privacy-service/)**
— the [architecture](https://abutlabs.github.io/jam-privacy-service/aztec/), the
[demo](https://abutlabs.github.io/jam-privacy-service/demo/), and
[our proposed JAM host calls](https://abutlabs.github.io/jam-privacy-service/jip/).

![An opening crawl: JAM PRIVACY, Episode ZK, A NEW PROOF](docs/images/intro.gif)

![Alice's and Bob's wallets side by side: Alice pays Bob in public, shields, and sends 40 privately; Bob finds the note and unshields; then the explorer read with no key, with Alice's keys and with Bob's viewing key](docs/images/demo.gif)

Recorded live on a six-validator JAM testnet. Nothing is mocked.

## Quickstart

Needs Rust, Python 3.11+, Node 20+ and Chrome. `bootstrap.sh` installs the rest at pinned
versions ([docs/TOOLS.md](docs/TOOLS.md)).

```sh
scripts/bootstrap.sh          # toolchain, sibling checkouts, both services built
nets/demo.sh up               # six JAM validators, services deployed, wallet on :19710 — about 75 s
nets/demo.sh check            # a shield, a private send, an unshield: 37 assertions
open "http://localhost:19710/?user=alice"        # Alice's wallet (?user=bob in another window)
open "http://localhost:19710/explorer.html"      # what the chain shows, and what a key reads in it
nets/demo.sh down
```

Other nets (lasair, mixed, Docker), the single moves, and the e2e and race tests:
[docs/DEMO.md](docs/DEMO.md).

## Status

| Milestone | |
| --- | --- |
| Private payments in Aztec's model, live on JAM — the recording above | **done** · [demo](https://abutlabs.github.io/jam-privacy-service/demo/) |
| Aztec v6's own node, unchanged, settling on JAM through our Ethereum-RPC facade | **done**, test-mode proofs · [architecture](https://abutlabs.github.io/jam-privacy-service/aztec/) |
| Aztec's transaction proofs verified on a live JAM net by Aztec's own verifier | **done** — needs our proposed host call: 3.46B gas with it, 11.4B (over two cores' budget) without · [proposal](https://abutlabs.github.io/jam-privacy-service/jip/) |
| The full stack: our wallet on Aztec's libraries, real proofs end to end, a prover node | **in progress** · [#71](https://github.com/abutlabs/jam-privacy-service/issues/71) |

## The proposal

Verifying a zero-knowledge proof is mostly elliptic-curve arithmetic, which JAM's VM meters as
ordinary code. We drafted a JAM Improvement Proposal adding five curve host calls — the
operations Ethereum ships as precompiles. Measured on a live net: verification 3 to 16×
cheaper, and Aztec's own client proofs become verifiable at all.
[The case and the numbers](https://abutlabs.github.io/jam-privacy-service/jip/) · [full
text](docs/jip/jip-final.md).

## Layout

| Path | What |
| --- | --- |
| `circuits/`, `services/`, `crates/` | the Noir circuit and the JAM services (privacy, public-balance, aztec-settlement) |
| `client/`, `indexer/` | the provers, the e2e tests, the films; the JIP-2 indexer behind the wallet and explorer |
| `tools/aztec-local/` | Aztec v6's released node on JAM: the Ethereum-RPC facade |
| `nets/` | the testnets: PolkaJam's, lasair's, Docker and mixed |
| `spikes/` | the measurement behind every number, each with its script |
| `docs/` | [DEMO.md](docs/DEMO.md) (the implementer's tour), [aztec/STACK.md](docs/aztec/STACK.md) (the port, mapped), [jip/](docs/jip/jip-final.md) (the proposal), [LEDGER.md](docs/LEDGER.md) (every step, with its evidence) |

Apache-2.0. An independent project — not affiliated with Aztec Labs or the Polkadot Fellowship.
