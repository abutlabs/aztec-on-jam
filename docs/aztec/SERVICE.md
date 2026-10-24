# The settlement service: Aztec's Rollup contract as refine and accumulate (#26)

The JAM service that takes Ethereum's place under Aztec (#28). It does what Aztec v6's Rollup
contract does (aztec-packages v6.0.0 `l1-contracts`: `ProposeLib`, `EpochProofLib`, `Inbox`,
`Outbox`), split the way JAM splits work: refine checks what can be checked from the package
alone (proofs, signatures, blobs) and reads no state; accumulate compares what refine vouched
for against the service's state and moves the tips. It is the privacy service's pattern
(refine verifies, accumulate compares roots) at Aztec's scale.

## State (service storage)

| Key | Holds | Ethereum's equivalent |
|---|---|---|
| `tip:pending`, `tip:proven` | checkpoint numbers and their archive roots | `RollupCore` tips |
| `cp:<n>` | a checkpoint's log: header hash, archive, slot, blob commitments hash, out hash, inbox position | `CheckpointLog` |
| `inbox:*` | the rolling-hash bucket ring (v6's streaming Inbox), the total message count | `Inbox.sol` |
| `outbox:<epoch>` | the epoch's out hash (the root of L2→L1 messages), and consumed leaves | `Outbox.sol` |
| `cfg` | chain id, version, vk tree root, protocol contracts hash, the epoch verification key, the committee | constructor arguments and the GSE |

## Two work-item kinds

**1. A checkpoint (propose).** Payload: the checkpoint header, the blob commitments and KZG
proofs, the proposer's signature and attestations. Extrinsics: the blobs (up to 6; JAM-NP's
200 KiB package limit makes them extrinsics, #36).

- *Refine*, from `ProposeLib.propose` and `validateHeader` (lines 184-400):
  - header fields that map to field elements are in range;
  - the coinbase is non-zero and the mana used is within the limit;
  - the DA fee is zero;
  - the blobs check against their commitments, batched (`spikes/kzg-gas`: 967,993,490 gas for 6);
  - the blobs hash matches the header;
  - the proposer's signature is valid (#39);
  - the blobs are exported as segments (`docs/aztec/BLOBS.md`).

  Output: the header hash, archive, slot, blob commitments hash, out hash and the inbox
  position claimed.
- *Accumulate*, the stateful half of `validateHeader` and `validateInboxConsumption`
  (441-490):
  - the parent archive equals `tip:pending`'s;
  - the slot is later than the last one and equals the current Aztec slot derived from JAM's
    slot;
  - the claimed inbox position is settled and does not go backwards;
  - then write `cp:<n>` and advance `tip:pending`.

**2. An epoch proof (`submitEpochRootProof`).** Payload: the root rollup proof (331 fields,
about 10.6 KB), its 113 public inputs, the batched blob opening, and the last checkpoint's
attestations.

- *Refine*:
  - verify the UltraHonk proof against `cfg`'s key: 180,791,299 gas at 2^24, measured on PolkaJam
    (#23);
  - verify the batched blob point evaluation (199,453,527 gas);
  - verify the attestations (#39).

  Output: the public inputs. Refine cannot read state, so it does not build the public
  inputs as Ethereum's `EpochProofLib` does (lines 290-434). It vouches that a valid proof
  exists for these.
- *Accumulate*, the comparison `EpochProofLib` does by construction:
  - `previous_archive_root` equals `tip:proven`'s archive;
  - every `checkpoint_header_hashes` entry equals the stored `cp:<n>`;
  - the inbox rolling hashes match the bucket ring;
  - `out_hash` matches the last checkpoint's;
  - `chain_id`, `version`, `vk_tree_root` and `protocol_contracts_hash` equal `cfg`;
  - the fees match the headers.

  Then advance `tip:proven`, write `outbox:<epoch>`, and pay fees by deferred transfer.

Gas per package, against a tiny package's 1e9 of refine: a checkpoint is about 968M plus its
signature checks; an epoch proof is about 380M plus its attestations (433M), 813M in all (`spikes/ecdsa-gas/RESULTS.md`). Each fits one package.
A full package (5e9) holds either with room to spare.

## Values Ethereum supplies, and JAM's answers

| Ethereum supplies | Used for | On JAM |
|---|---|---|
| `block.chainid` | `chain_id` in every header, message hashes, the epoch proof | a constant in `cfg`, fixed at deployment (one per JAM chain and service) |
| `block.timestamp`, L1 slots | Aztec's slot and timestamp checks | Aztec slot = f(JAM timeslot): JAM's 6 s slot as Aztec's clock, with Aztec's slot duration a multiple of it |
| `blobhash`, point evaluation | blobs | refine's KZG checks and exported segments (`BLOBS.md`) |
| `ecrecover` | proposer and attestations | PVM ECDSA (#39) |
| L1 senders and recipients (20 bytes) | L1↔L2 messages | JAM service ids, as 20-byte values; a message from another service arrives as a deferred transfer whose memo carries the content |
| The fee asset and its portal | fees, fee juice | JAM's native balance, bridged by deferred transfers |
| `block.basefee`, `blobbasefee` | the mana minimum fee | a fee rule set by the service (no L1 gas to track) |
| Finality | the proven tip is final when Ethereum finalizes | the proven tip is final when JAM finalizes (GRANDPA) the block that accumulated it |

## Who proposes, and the committee

Aztec's sequencers and committee are chosen by its staking contracts (GSE) and Ethereum's
randomness. A first version keeps Aztec's committee logic with a fixed set in `cfg`, and leaves
staking, rewards, slashing and governance for later (`BOUNDARY.md` lists them as optional).
Whether JAM's own guarantors could replace Aztec's committee is a question for #30: they
already re-execute refine, so an epoch's validity is checked by JAM's auditors, not only by
the proof.

## Built: the checkpoint path

`services/aztec-settlement` (#49) is the propose path:
- **Refine** checks the header's stateless conditions, the blobs against their commitments
  (batched), and the header's blobs hash. It exports the blobs and outputs the header hash and
  the values accumulate needs.
- **Accumulate** checks the parent archive, the slot, the timestamp, the fee and the inbox
  hash, then writes `cp:<n>` and `tip:pending`, advancing the epoch's blob commitments hash.

On PolkaJam it accepts three chained checkpoints made by Aztec's released v6 packages, in order,
storing Aztec's own header hashes. It refuses them out of order, and refuses tampered headers,
blobs and timestamps (`services/aztec-settlement/RESULTS.md`). Its encodings match all six of
Aztec's v6 L1 fixtures (`crates/aztec-l1`). The inbox (#50), the epoch path (#51) and the
signatures (#54) follow.

## Built first: the skeleton

`spikes/settlement/` (ledger 51) is the epoch-proof path, deployed on a PolkaJam testnet:
- **Refine** verifies an Aztec-shaped epoch proof (plain UltraHonk, Keccak transcript, 113
  public inputs; #23's 2^16 every-relation circuit) and outputs its first two public inputs
  as `previous_archive_root` and `end_archive_root`.
- **Accumulate** keeps `tip`: a verified proof advances it only from the tip, and the first
  accepted proof anchors the chain.

| sent | expected | refine gas | tip after | proven | outcome |
|---|---|---|---|---|---|
| A (1 → 2) | advance | 170,427,917 | 2 | 1 | advanced |
| A again | stale | 170,427,917 | 2 | 1 | stale |
| C (5 → 6) | stale | 170,258,342 | 2 | 1 | stale |
| B (2 → 3) | advance | 170,262,144 | 3 | 2 | advanced |
| B, a public input altered | fails in refine | 8,330,413 | 3 | 2 | failed |

The rest of the design (checkpoints with blobs and signatures, the Inbox ring, the Outbox,
fees) is specified above, and its per-package costs are measured (#23, #37, #39).
