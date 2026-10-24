# The settlement service on PolkaJam: checkpoints (#49), the Inbox (#50), epoch proofs (#51)

`services/aztec-settlement/measure.sh`, 2026-10-24, Apple M-series Mac, polkajam-testnet v0.8.0
(six validators, the PVM interpreter), via `spikes/lib/polkajam.sh`. No JIP: all of it is PVM
code.

**Against Aztec's own encodings, two ways:**

1. **Aztec v6's L1 fixtures** (`crates/aztec-l1/fixtures/v6`, from aztec-packages v6.0.0
   0219f11). `cargo test -p aztec-l1` checks, for all six, that the service's header hash
   (`ProposedHeaderLib.hash`) and blobs hash (`BlobLib.validateBlobs`) equal the fixtures'. It
   also checks that each second checkpoint builds on its first, and that the stateless header
   checks pass, with a zero coinbase, an out-of-field hash and a wrong blobs hash refused. The
   fixtures hold blob commitments but not blob data (`write_json.ts`), so they can't go through
   refine's blob check.
2. **Checkpoints made by Aztec's released v6 packages** (`tools/aztec-gen`, `@aztec-labs/stdlib`
   and `@aztec-labs/blob-lib` 6.0.0, unchanged): `Checkpoint.random`, its blobs as Aztec's
   publisher makes them (`getBlobsPerL1Block(checkpoint.toBlobFields())`), their KZG proofs, and
   the header hashed by Aztec's `CheckpointHeader.hash()`. Three chained checkpoints, with 1, 2
   and 1 blobs. These are what the testnet ran:

| case | blobs | refine gas | verdict | expected |
|---|---|---|---|---|
| checkpoint 2 before 1 | 2 | 404,598,679 | 10, wrong parent | 10 |
| checkpoint 1 | 1 | 262,397,812 | 1, accepted | 1 |
| checkpoint 1 again | 1 | 262,397,812 | 10, wrong parent | 10 |
| checkpoint 2, blobsHash altered | 2 | 319,065 | 2, refused in refine (header) | 2 |
| checkpoint 2, a blob's byte altered | 2 | 404,534,856 | 3, refused in refine (blobs) | 3 |
| checkpoint 2, timestamp altered | 2 | 404,598,679 | 13, wrong timestamp | 13 |
| checkpoint 2 | 2 | 404,598,679 | 1, accepted | 1 |
| checkpoint 3 | 1 | 262,397,812 | 1, accepted | 1 |

The stored `cp:1`, `cp:2` and `cp:3` header hashes equal the ones Aztec's code computed, and
`tip:pending` is checkpoint 3.

**Gas.** A checkpoint costs about 141M a blob plus 122M (the batched KZG check dominates), so
Aztec's maximum of 6 blobs is about 968M (`spikes/kzg-gas`, #37). That fits a tiny package
(1e9) and a fifth of a full one.

**What JAM changes from Ethereum's `validateHeader`,** each recorded in the service's source:
- **The slot** must be after the tip's and not ahead of the Aztec clock. On Ethereum it equals
  the current L1 slot, but a JAM package is accumulated a slot or more after it is refined. The
  clock is JAM's timeslot as Unix time (GP 0.8.0 `overview.tex:91`), against Aztec's genesis time
  and slot length in `cfg`.
- **The mana fee** must equal the service's fixed minimum. Aztec's L1 fee oracle tracks Ethereum
  gas, which has no JAM meaning.
- **The blobs** are checked in refine against their commitments. Ethereum's consensus does that
  for a contract, and on JAM the service must.
- **Not yet here:** the proposer's signature and attestations (#54).

**A JAM detail found on the way.** A refused work item must still export its declared number
of segments. Otherwise the GP replaces its output with an error (`work_packages_and_reports.tex:215`)
and the reason for the refusal is lost, so refine exports zeros.

## The Inbox (#50)

`services/aztec-settlement/inbox.sh`, the same machine and testnet. An L1→L2 message is a
deferred transfer to the settlement service, from any JAM service; here `spikes/inbox-sender`
stands in for an L1 contract. The memo carries the recipient, content, secret hash and version,
and the sender is the sending service. Accumulate hashes each message into Aztec's leaf and
absorbs it into the current bucket. A new bucket opens each JAM timeslot, as Ethereum's Inbox
opens one each L1 block. The messages are kept at `inbox:m:<index>` for Aztec's node.

**Against Aztec's own code.** The fixtures don't record the rollup version their leaves hash
over: Aztec's Solidity tests overwrite the header's inbox hash with their own Inbox's
(`RollupBase.sol`). So the reference is Aztec's released TypeScript, `L1ToL2Message.hash()` and
`accumulateInboxRollingHash` (`tools/aztec-gen/inbox.mjs`). It is also a native test
(`crates/aztec-l1/fixtures/generated/inbox.json`).

Two packages from the sender: 3 messages, then 2, one of them for rollup version 2.

```
bucket 1: rolling hash equals Aztec's, total 3 (Aztec 3)
bucket 2: rolling hash equals Aztec's, total 4 (Aztec 4)
buckets 2, refused messages 1
message 0: leaf equals Aztec's
message 1: leaf equals Aztec's
message 2: leaf equals Aztec's
message 3: leaf equals Aztec's
```

**Consumption** (`ProposeLib.validateInboxConsumption`). The checkpoints come from Aztec's
packages, at a slot whose cutoff (`TimeLib.getInboxCutoffTimestamp`, with JAM's 6 s slot as the
L1 slot) is past bucket 2's opening:

| checkpoint | verdict | expected |
|---|---|---|
| skips bucket 2, past its cutoff | 20, unconsumed messages | 20 |
| consumes buckets 1–2 | 1, accepted | 1 |
| then bucket 1 (behind the parent) | 18, behind the parent | 18 |
| bucket 2 named, bucket 1's hash | 15, wrong inbox hash | 15 |
| bucket 9, which doesn't exist | 16, no bucket | 16 |
| then bucket 2 (nothing new) | 1, accepted | 1 |

`cp:1` and `cp:2` each record 4 messages consumed, up to bucket 2. The settlement service's
accumulate costs 263,367 gas for 3 messages and 111,828 for 2 (PolkaJam telemetry).

**Not ported:** Ethereum's Inbox keeps a 4,096-bucket ring and evicts a bucket once it is proven
consumed. Here every bucket is kept. A refused message's transfer has already moved its 1 unit,
which stays with the service: a JAM transfer can't be reverted by its recipient.

## Epoch proofs (#51)

`services/aztec-settlement/epoch.sh`, the same machine and testnet.

- **The proof.** Aztec's prover isn't available here, so it is a genuine bb 6.0.0 UltraHonk
  proof in the epoch proof's shape: plain, Keccak transcript, the 113 public inputs all free
  (`testdata/epoch-circuit`, 2^16 gates, 0.6 s to prove).
- **The public inputs and blob opening come from Aztec's code.** `tools/aztec-gen/epoch.mjs`
  uses `RootRollupPublicInputs.toFields()` over the two checkpoints, and
  `BatchedBlobAccumulator.batch` for the batched opening.
- **What each side checks.** Refine verifies the proof and the opening, and that the last six
  public inputs are the opening's (`EpochProofLib`'s limbs). Accumulate compares the other 107
  with what `computeEpochProofPublicInputs` would assemble from the stored checkpoints and
  `cfg`, after `assertAcceptable`'s range checks.

| item | refine gas | verdict | expected |
|---|---|---|---|
| checkpoint a | 262,397,812 | 1, accepted | 1 |
| checkpoint b | 404,598,679 | 1, accepted | 1 |
| epoch proof, vk tree root wrong (a valid proof) | 359,284,024 | 36, wrong public input (accumulate) | 36 |
| epoch proof, a public input changed after proving | 8,559,249 | 4, bad proof (refine) | 4 |
| epoch proof, the blob opening's z altered | 358,999,149 | 5, bad blob opening (refine) | 5 |
| epoch proof | 359,321,493 | 1, accepted | 1 |
| epoch proof again | 359,321,493 | 37, already proven | 37 |

```
tip:proven checkpoint 2, archive equals checkpoint b's
outbox: out hash equals checkpoint b's, 2 checkpoints
cp:2 blob commitments hash equals Aztec's BatchedBlob's
```

**Gas.** An epoch proof's refine costs 359M: the UltraHonk check at 2^16 plus the point
evaluation (199M). Aztec's real proof is 2^24, which #23 measured at 180.8M for the check
alone, so a real epoch proof is about 380M, plus 433M for the attestations once #54 adds them.
That is 813M in all, within a tiny package.

**Not ported:** pruning unproven checkpoints when the proof window closes, prover rewards, and
the attestations (#54). The proof window itself is checked.

