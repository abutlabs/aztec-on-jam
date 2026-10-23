# Aztec's Ethereum boundary, and the seams a JAM backend needs (#25)

Read from `aztec-node` at 1ac42dc (2026-09-07; `yarn-project/`) and `aztec-packages` v6
nightly 2026-10-21 (`l1-contracts/`, protocol circuits). The direction (#28): Aztec's software
runs unchanged above the settlement layer, and JAM replaces Ethereum below it. This file says
what touches Ethereum, what already sits behind an interface, and the few seams Aztec would
need so a JAM backend is a plug-in rather than a fork. Line references are to those commits;
the key ones were re-checked when this file was written (ledger 39).

## What runs unchanged

- **The PXE.** It takes `l1ChainId`, `rollupVersion` and `l1ContractAddresses` from the node's
  `getNodeInfo` (`pxe/src/entrypoints/server/utils.ts:40`), only to label its store and fill
  `TxContext`. Nothing in it is anchored to Ethereum block data: its anchor is an L2 block
  header.
- **The circuits**, kernels and rollup: they take chain id, version, slots, fees, roots and
  hashes as values. Ethereum semantics remain in two places: blobs as EIP-4844 KZG (handled
  in refine, `docs/aztec/BLOBS.md`) and 20-byte addresses (coinbase, fee recipients, L1↔L2
  message parties), which a JAM service supplies as values.
- **Proving, world state, P2P, the simulator**: no Ethereum calls of their own.
- **Proof of concept that a node runs without Ethereum:** Aztec's TXE builds an
  `AztecNodeService` with no L1, from `TXEArchiver` and `MockEpochCache`
  (`txe/src/state_machine/index.ts`).

## What touches Ethereum

| Component | What it does on Ethereum | Behind an interface? | Lines a JAM version replaces |
|---|---|---|---|
| `RollupContract` (`ethereum/src/contracts/rollup.ts`, 1,502 lines) | every read and write of the Rollup contract, over viem | **No**: a concrete class (`:288`) the others hold | the reads callers use (below) |
| Sequencer publisher (`sequencer-client/src/publisher/`) | propose a checkpoint with its blobs, invalidate, prune, governance and slashing votes, bundled through Multicall3 | **No**: `SequencerPublisher` is a class (`sequencer-publisher.ts:167`), typed directly in `sequencer.ts:768` | 2,876 |
| Archiver's L1 side (`archiver/src/l1/`, `modules/`) | follow Rollup and Inbox events, decode `propose` calldata, fetch blobs, outbox roots, attestation checks | Outward yes (`L2BlockSource` `stdlib/src/block/l2_block_source.ts:118`, `L1ToL2MessageSource`); inward **no** (viem clients, `RollupContract`) | 4,944 (`l1/` and `modules/`) |
| Epoch cache (`epoch-cache/src/epoch_cache.ts`) | committee, sample seed, escape hatch, L1 timing | **Yes**: `EpochCacheInterface` (`:60`); `TestEpochCache`, TXE's `MockEpochCache` | 597 |
| Prover node publisher (`prover-node/src/prover-node-publisher.ts`) | submit the epoch root proof with its blob inputs | **Yes**: `PublisherLike` (`proof-publishing-service.ts:59`) | 473 |
| Blob client (`blob-client/`) | Ethereum beacon API for blobs | **Yes**: `BlobClientInterface` (`client/interface.ts:25`) | 2,002 (the package) |
| Slasher (`slasher/`) | slashing rounds on Ethereum | **Yes**: `SlasherClientInterface` (`slasher_client_interface.ts:6`), `NullSlasherClient` | none for a first version |
| Global variables, fees (`sequencer-client/src/global_variable_builder/`) | chain id from the viem client, mana fee from the Rollup | Yes (`GlobalVariableBuilder`, `FeeProvider`), but the stdlib interface imports an Ethereum type | 351 |
| Wiring (`aztec-node/src/factory.ts`, `sequencer-client/src/client/`) | build viem clients, read the Registry, construct the above | no; it is the composition root | rewire |
| Transaction machinery (`ethereum/src/l1_tx_utils/`, `node-lib/`) | gas, nonces, speed-up, cancel | generic over `L1TxUtils` | replaced, not ported |

In all, about 11k lines of TypeScript have a JAM counterpart (non-test lines, counted with
`wc -l` at the commit above), and the `ethereum` package (11,277) is replaced rather than
ported. Timing is Ethereum-shaped throughout
(`L1RollupConstants`: Ethereum's slot time, L1 genesis, start block), so a JAM backend maps
JAM's 6-second slots onto those fields.

**What the callers use.** The sequencer calls 14 methods of its publisher:
`enqueueProposeCheckpoint`, `validateCheckpointHeader`, `canProposeAt`, `sendRequests`,
`sendRequestsAt`, `clearPendingRequests`, `interrupt`, `getSenderAddress`,
`enqueueInvalidateCheckpoint`, `simulateInvalidateCheckpoint`, `enqueuePruneIfPrunable`,
`enqueueGovernanceCastSignal`, `enqueueSlashingActions`, `getFeeAssetPriceModifier`. The
archiver, epoch cache and prover node read the Rollup through 14 methods: `getTips`,
`getCheckpoint`, `status`, `archiveAt`, `getProvenCheckpointNumber`, `canPruneAtTime`,
`getEpochProofPublicInputs`, `getProofSubmissionEpochs`, `getCurrentEpochNumber`,
`getCommitteeAt`, `getAttesters`, `getSampleSeedAt`, `isEscapeHatchOpen`,
`getSpecificProverRewardsForEpoch`.

## The seams: a draft proposal to Aztec (not sent)

Drafted for a later phase; nothing is posted from the loop (RALPH Phase 3). Four changes,
all refactors inside Aztec's code with Ethereum as the only implementation at first:

1. **`SettlementPublisher`**, an interface extracted from `SequencerPublisher` with the 14
   methods above. The Ethereum class implements it unchanged; `sequencer.ts` and the
   checkpoint proposal job take the interface. Governance, slashing and the fee-asset
   modifier become optional members (a settlement layer may not have them).
2. **`SettlementReader`**, an interface over the 14 Rollup reads, which `RollupContract`
   implements. The archiver, epoch cache and prover node take it instead of the class.
3. **`SettlementSource`** for the archiver's ingest: checkpoints with their data, Inbox
   messages, proven-tip and finality events. Today's L1 synchroniser becomes its Ethereum
   implementation.
4. **Settlement-neutral types in `stdlib`**: `L1ContractAddresses` behind a settlement
   descriptor, `getL1Timestamp` as `getSettlementTimestamp`, inbox buckets keyed by
   settlement time, `SimulationOverridesPlan` out of the `GlobalVariableBuilder` interface.

With those four, a JAM backend is a package beside `ethereum`: a `SettlementPublisher` that
submits checkpoints and epoch proofs as work-packages (JIP-2), a `SettlementReader` and
`SettlementSource` that read the settlement service's storage and reports (JIP-2), a
`BlobClientInterface` over JAM's exported segments (`docs/aztec/BLOBS.md`), and wiring.
Every other package, the PXE included, is used as released.

**Why Aztec might want it anyway:** the same seams serve any non-Ethereum settlement, and they
make the sequencer and archiver testable without anvil. Aztec's TXE already works around
their absence.

## Open

- The settlement service these interfaces talk to: #26.
- Whether any seam is already planned upstream: check Aztec's issues and the v6 branch before
  a proposal is sent (#27).
