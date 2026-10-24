# How an unchanged Aztec v6 node drives JAM (#52)

**Decision: an Ethereum JSON-RPC facade.** It is a separate process that answers Aztec's node
as its L1 would, backed by the settlement service on JAM: reads over JIP-2, writes as work
packages. Aztec's packages run as released (`@aztec-labs/aztec` 6.0.0), with their own L1
client, ABIs and archiver. The four seams of `BOUNDARY.md` would be cleaner, but they are a patch
to Aztec's code, and keeping one alive is what Aodh asked to avoid. They stay a proposal for
Aztec, for a later phase.

## The record

`tools/aztec-local`: Aztec v6's own local network (`aztec start --local-network`, Foundry
1.8.5 as Aztec pins it, `aztec-node` `bootstrap.sh:19`). Its L1 endpoint is `rpc-proxy.mjs`,
which forwards to anvil and records every request. `analyze.mjs` names each one against the
node's own contract addresses (`node_getNodeInfo`) and Aztec's ABIs
(`@aztec-foundation/l1-artifacts`). Tool output is `docs/aztec/node-l1-record.json`. The run:
- **Deployment:** 14 contracts deployed directly and 6 through the deterministic CREATE2 deployer
  (`0x4e59b448…`), then setup calls.
- **Checkpoints:** four, with the test accounts' transactions, each proposed and synced back by
  the archiver.
- **Proving:** the epochs, by the local network's cheat codes.
- **Then:** 11.5 minutes of steady state.

Every request:

| Kind | Methods (count) | In steady state |
|---|---|---|
| **The chain** | `eth_getBlockByNumber` (4,877), `eth_blockNumber` (49), `eth_chainId` (9), `eth_getBlockByHash` (3) | 405 block requests a minute: every component polls the head |
| **Contract reads** | `eth_call` (3,550): 42 functions, below | 286 a minute, 95% of them `Rollup.getTips` |
| **Writes** | `eth_sendRawTransaction` (39): 14 deployments and 20 setup calls, then **one per checkpoint**, a blob transaction (EIP-4844) to Multicall3 wrapping `Rollup.propose` | 1 a checkpoint |
| **Write support** | `eth_estimateGas` (4, `Rollup.validateBlobs`), `eth_simulateV1` (4), `eth_getTransactionCount` (18), `eth_feeHistory` (26), `eth_blobBaseFee` (24), `eth_maxPriorityFeePerGas` (24), `eth_gasPrice` (2), `eth_fillTransaction` (1), `eth_getBalance` (18), `eth_getAccountInfo` (33) | about 1 a minute each |
| **Reading writes back** | `eth_getTransactionReceipt` (75), `eth_getTransactionByHash` (10), `debug_traceTransaction` (1), `trace_transaction` (1) | per checkpoint. The archiver decodes `propose` from the transaction's input, using the trace to find it inside Multicall3. v6 requires a trace-capable endpoint (aztec-node v6.0.0 release notes, #511) |
| **Events** | `eth_getLogs` (8): `Rollup.CheckpointProposed` (4), and at startup `SlasherUpdated`, `OwnershipTransferred`, `Registry.CanonicalRollupUpdated` | `CheckpointProposed` only |
| **Storage** | `eth_getStorageAt` (20), all on the Rollup | under 1 a minute |
| **anvil's cheat codes**, local network only | `hardhat_setStorageAt` (8), `evm_mine` (4), `evm_setNextBlockTimestamp` (4), `anvil_*` (51), `hardhat_mine` (1) | how the local network "proves" an epoch: it writes the proven tip and the Outbox into storage |

The 42 functions read, by count:
- **Rollup, state:** `getTips` 3,348, `getCheckpoint` 16, `canPruneAtTime` 15,
  `getPendingCheckpointNumber` 12, `status` 9, `getProvenCheckpointNumber` 9, `archiveAt` 5.
- **Rollup, time:** `getCurrentSlot` 8, `getTimestampForSlot` 7, `getGenesisTime`,
  `getSlotDuration`, `getEpochDuration`, `L1_BLOCK_AT_GENESIS`, `getProofSubmissionEpochs`.
- **Rollup, fees:** `getManaMinFeeAt` 30, `getL1FeesAt` 12, `getProtocolFeeMargin`,
  `getManaLimit`, `getManaTarget`, `getProvingCostPerManaInEth`.
- **Rollup, committee:** `getCommitteeAt`, `getSampleSeedAt`, `getTargetCommitteeSize`,
  `getEscapeHatch`, `getLagInEpochsForValidatorSet`, `getLagInEpochsForRandao`, `getSlasher`.
- **Rollup, wiring:** `getVersion`, `getInbox`, `getOutbox`, `getFeeAsset`,
  `getFeeAssetPortal`, `getRewardDistributor`, `getStakingAsset`, `getGSE`.
- **Inbox:** `getState` 9, `getBucketAtOrBeforeTotal` 4.
- **Others:** `Registry.getRollup`, `Registry.getGovernance`, `Governance.governanceProposer`,
  `FeeJuice.owner`.

## The facade, method by method

| What Aztec asks | The facade's answer | Backed by |
|---|---|---|
| Blocks, head, chain id | One "L1 block" per finalized JAM block: number, hash and timestamp (JAM's timeslot as Unix time, `overview.tex:91`). The chain id is `cfg`'s | JIP-2 head and block subscriptions |
| Rollup and Inbox state reads (about 15) | ABI-encoded from the service's storage: `tip:pending`, `tip:proven`, `cp:<n>`, `inbox:b:<n>`, `inbox:seq` | JIP-2 service storage reads |
| Time, configuration and wiring reads (about 20) | Constants from `cfg`, and addresses chosen for the facade's own "contracts" | the facade's config |
| Fee reads | The service's fee rule: a fixed `getManaMinFeeAt`, and zero L1 fees | `cfg` |
| Committee reads | A fixed committee (`cfg`, #54), its seed constant | `cfg` |
| `eth_sendRawTransaction` of Multicall3 → `propose`, with blobs | Decode the transaction and its blob sidecar, take `propose`'s header, archive and bucket hint and the blobs, submit a work package: kind 1, blobs as extrinsics. Keep the transaction for later reads | JIP-2 submission |
| `submitEpochRootProof` (the prover node, on a real network) | Decode it and submit work-item kind 2 | JIP-2 submission |
| Estimate, simulate, nonce, fees, balance | Run the service's refine-side checks locally (`aztec-l1` as a library) for simulations; constants and a nonce counter for the rest | local |
| Receipts, transaction by hash, traces | From the kept transaction, once accumulate's verdict is visible in storage: status 1 if accepted, 0 if refused. A minimal call trace places `propose` inside Multicall3 | JIP-2 storage reads |
| `CheckpointProposed` logs | Synthesized for each new `cp:<n>`, in Aztec's event encoding | JIP-2 storage reads |
| Deployment | Not done: the node starts with the facade's contract addresses configured, so it deploys nothing | — |
| Cheat codes | Not offered. Proving takes a real prover (#51's epoch path), or the proven tip lags, which Aztec tolerates | — |

**Size.** About 40 read handlers, each one storage read and an ABI encoding. One write decoder.
The block, receipt and log synthesis. viem, which Aztec's node already uses, does the RLP and
ABI work. This is an estimate, a few thousand lines of TypeScript.

**What it costs per Aztec release:**
- **Changed L1 interfaces:** the facade follows them. That is the change `RELEASES.md` measured
  (v5 → v6: 80 lines in `rollup.ts`, 379 in the publisher), now in ABI handlers instead of a
  patch to Aztec.
- **New RPC expectations:** new ones, such as v6's traces, show up in the record.
  `tools/aztec-local` is the check to re-run each release.

**Against the seams.** The seams avoid emulating Ethereum (blocks, receipts, traces) and
would be smaller. But they change Aztec's TypeScript, and every release would have to be merged
into a patched copy until Aztec adopted them. The facade changes nothing of Aztec's.

## What #53 builds

The facade (`tools/jam-l1`), in this order:
1. **The reads, blocks and logs,** so that Aztec's archiver syncs the checkpoints the service
   holds.
2. **The propose write,** so that Aztec's sequencer settles on JAM.
3. **Receipts and traces,** so that the archiver reads its own proposals back.

The node runs with `--node --archiver --sequencer`, not `--local-network`, against the facade's
addresses.

## Built: Aztec v6's released node settling on JAM (#53)

`tools/aztec-local/on-jam.sh --node --sequencer` starts three things:
- a PolkaJam testnet with the settlement service configured for the node's genesis
  (`services/aztec-settlement/devnet.sh`);
- the facade (`tools/aztec-local/jam-l1.mjs`) as the node's L1;
- the node, `@aztec-labs/aztec` 6.0.0 as released.

`node send-tx.mjs` then sends transactions through Aztec's own PXE (`EmbeddedWallet`), the way the
local network's start does it.

Run on 2026-10-24:

```
publish class:    tx 0x18d3ee3e…  status proposed  block 4  after 15 s
publish instance: tx 0x173cd3f4…  status proposed  block 5  after 20 s
class publicly registered: true | instance published: true
archiver: Downloaded checkpoint 1–3: 0 tx; checkpoint 4: 2 tx; checkpoints 5–7: 0 tx
node_getTxReceipt: checkpointed, block 4 / checkpointed, block 5
facade: 7 checkpoints accepted by the settlement service, 0 refused, 7 replacement
        transactions folded into their originals, 0 errors
```

Every checkpoint takes the full path:
1. Aztec's sequencer builds it and proposes it to "L1": a blob transaction to Multicall3.
2. The facade relays it as a work package (kind 1, blobs as extrinsics, KZG proofs
   recomputed).
3. The settlement service checks it in refine and chains it in accumulate.
4. The facade sees `cp:<n>` and emits `CheckpointProposed`, with the payload digest and
   attestations hash computed by Aztec's own functions.
5. Aztec's archiver downloads it, verifies the calldata against those hashes, and reads the
   blobs from the file store the sequencer wrote.

**What the facade had to get right**, each found by a run:
- **Genesis:** the node stands by until the Rollup's genesis matches its own.
- **The deployment log:** the archiver probes the Rollup's deployment `OwnershipTransferred`.
- **The exact `CheckpointLog` shape.**
- **Raw storage reads:** `isEpochSetUp`, and the STF tips slot.
- **State overrides:** `eth_call`'s and `eth_simulateV1`'s, which the sequencer uses to simulate
  pipelined proposals, with a block-time override.
- **Reverts** as JSON-RPC error 3, with Aztec's custom errors ABI-encoded.
- **Nonces:** `latest` counts only included transactions.
- **Replacements:** a second transaction for a pending nonce replaces the first, so one
  checkpoint is relayed once.
- **The event's hashes,** which the archiver checks the calldata against.

**Not yet, and why:**
- **Test mode** (`PROVER_REAL_PROOFS=false`), as the local network runs: transactions' client
  proofs aren't verified.
- **No prover,** so the proven tip stays at 0. The epoch path is built and tested with a real
  bb 6 proof (#51). A prover node on this net needs Aztec's real proving, which is hardware-bound
  here.
- **No committee:** the target committee size is 0, as in the local network. Signatures are
  #54.
- **P2P off,** with one sequencer.
