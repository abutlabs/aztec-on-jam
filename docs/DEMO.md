# Ten minutes with Aztec on JAM

Aztec's software as released, settling on a JAM chain instead of Ethereum. This page is for a
JAM implementer who wants to see it run, know what the chain actually does, and decide whether
to believe it. What is Aztec's code and what is ours: [aztec/STACK.md](aztec/STACK.md).

## What happens in one payment

1. **The sender proves, in their browser.** Our wallet (`wallet/`) drives Aztec's wallet
   libraries: Aztec's private execution environment (PXE) runs the transaction and Aztec's
   prover (bb.js) makes its client proof in the tab, 8 to 16 s on a laptop.
2. **Aztec's node checks the proof and sequences it.** The unchanged `@aztec-labs/aztec` 6.0.0
   node verifies the client proof on arrival (a tampered one is refused in 0.14 s) and builds a
   block. Blocks are grouped into checkpoints; each checkpoint is proposed to "Ethereum".
3. **Our sidecar stands where Ethereum was.** `tools/aztec-local/jam-l1.mjs` answers Aztec's
   node as Ethereum's JSON-RPC would, from JAM's state, and turns each proposal into a JAM work
   package for the settlement service: the header, the blob data, the committee's signatures.
4. **Refine verifies; accumulate records.** The settlement service (`services/aztec-settlement`,
   Aztec's Rollup contract rewritten as a JAM service) checks the blobs against their KZG
   commitments and the signatures in refine: 64M gas with the proposed curve host calls, 251M
   without. Accumulate checks the header against the chain it holds (parent archive, slot, fees,
   inbox) and advances the pending tip. GRANDPA finalizes it, and Aztec's node sees the
   checkpoint as settled.
5. **Aztec's prover proves the epoch.** Aztec's prover (a node that plans the work, a broker
   that queues it, agents that prove it) proves the whole epoch — every transaction, block and
   checkpoint — into one root rollup proof: 60 to 171 s on a 10-core laptop. Public execution is
   proved by Aztec's AVM prover in Aztec's own Docker image.
6. **JAM verifies the epoch proof.** The sidecar relays it as a second kind of work item; refine
   verifies the UltraHonk proof (70.7M gas, 0.55 s) against Aztec's public inputs, recomputed from
   what the service holds; accumulate advances the proven tip. The payment is final and proven.
7. **The receiver finds the note** with their own key, by the tags of senders they registered:
   Aztec's note discovery. The chain holds no sender, receiver or amount.

## Run it

```sh
scripts/bootstrap.sh                                  # the pinned toolchain, Aztec's packages, the wallet
DEVNET_NET=jb-lasair6 tools/aztec-local/on-jam.sh     # six lasair validators, the service, the sidecar, Aztec's node and prover
```

It prints each step and, after about two minutes, the wallet's address. Leave `DEVNET_NET`
out for PolkaJam's testnet (no curve calls: the same service, more gas). Docker runs the
public-execution prover; without it, epochs with public calls stay unproven and the chain runs
on. `tools/aztec-local/on-jam.sh down` stops everything from another terminal.

## Check it

```sh
node tools/aztec-local/send-tx.mjs     # a fresh account deploys Aztec's Token, mints, transfers; balances asserted
node wallet/e2e.mjs                    # Alice and Bob in two browsers: mint, unshield, shield, send, find, reload
node wallet/film/demo.mjs              # the film on the demo page, recorded from a live run
```

The wallet's live view shows who is doing what: this tab, Aztec's node, the sidecar, JAM's
validators, Aztec's prover, each with a timer while it works, and every action's timeline from
the proof in the tab to the epoch proof verified on JAM.

## Where the numbers come from

Every figure on this page has its command in the repository and its run in
[LEDGER.md](LEDGER.md). Laptop figures are from an Apple M1 Pro with 16 GB; gas is the
protocol's and does not depend on the machine.

## What came before

The first proof of concept — private payments in Aztec's model with our own Noir circuit, a
verifier service, a Python wallet, an indexer and an explorer — is retired. It is kept whole on
the branch [`archive/first-demo`](https://github.com/abutlabs/aztec-on-jam/tree/archive/first-demo).
Its verifier service and circuit stay in `services/privacy` and `circuits/` as the proposal's
benchmark: an UltraHonk proof at Aztec's sizes, checked in refine.
