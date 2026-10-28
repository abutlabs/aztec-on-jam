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
   proved by Aztec's AVM prover in Aztec's own Docker image. Only blocks with transactions are
   built, so only they are proved: see [Proving only what was used](#proving-only-what-was-used).
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

### Proving only what was used

Hardware is this demo's bottleneck, so the chain does not build a block, and the prover does not
prove one, unless it holds a transaction. Aztec's sequencer can build a checkpoint every slot,
empty or not, and an empty checkpoint costs the prover as much of an epoch as a full one. With
every slot built, the chain made an epoch (four 36 s slots) every 2.4 minutes while one prover agent
took 6.5 to 9 minutes to prove one (measured 2026-10-28 with a backlog queued and browser tests
proving on the same machine): the backlog only grew, a payment waited past 22 minutes for its epoch
proof, and the load slowed JAM's confirmations until Aztec's node abandoned checkpoints and pruned
their blocks.

So `on-jam.sh` turns empty checkpoints off through the node's admin API
(`tools/aztec-local/empty-checkpoints.mjs`, using Aztec's own admin client), with two exceptions:

- **Block 1.** A transaction built before block 1 is valid only in block 1, so the chain starts
  with empty checkpoints on, and they go off once block 1 is in.
- **A long idle.** A transaction expires 24 hours after the block it is anchored to
  (`MAX_TX_LIFETIME`). A chain with no block for that long would never take a transaction again,
  so after 12 hours with no block (`IDLE_HOURS`) it builds one empty checkpoint.

`EMPTY_CHECKPOINTS=1 tools/aztec-local/on-jam.sh` builds and proves every slot instead, as Aztec's
own networks do; the log is `out/aztec-devnet/empty-checkpoints.log`.

## Check it

```sh
node tools/aztec-local/send-tx.mjs     # a fresh account deploys Aztec's Token, mints, transfers; balances asserted
node wallet/e2e.mjs                    # Alice and Bob in two browsers: mint, unshield, shield, send, find, then the explorer, reload
node wallet/view-keys.mjs              # viewing keys in a fresh PXE: balances and payments received, no spending
node wallet/film/demo.mjs              # the film on the demo page, recorded from a live run
```

The wallet's live view shows who is doing what: this tab, Aztec's node, the sidecar, JAM's
validators, Aztec's prover, each with a timer while it works, and every action's timeline from
the proof in the tab to the epoch proof verified on JAM.

Its **Explorer ↗** link opens the Imperial Ledger (port 19730), a block explorer with a search for addresses, transactions and blocks, on what the chain shows anyone:
each transaction's commitments, nullifiers and ciphertext, with no sender, recipient or amount;
each checkpoint's and epoch proof's work package on JAM; public balances; JAM's own chain. Paste
an account's viewing keys from the wallet (Receive → Show viewing keys) and the explorer's own
PXE adds what those keys reveal: the balance, the payments received, and which of the public
transactions they were. The keys cannot spend; the signing key never leaves the wallet.

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
