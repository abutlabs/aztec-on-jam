# The checkpoint path on PolkaJam (#49)

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
| checkpoint 2 before 1 | 2 | 404,591,891 | 10, wrong parent | 10 |
| checkpoint 1 | 1 | 262,393,344 | 1, accepted | 1 |
| checkpoint 1 again | 1 | 262,393,344 | 10, wrong parent | 10 |
| checkpoint 2, blobsHash altered | 2 | 315,542 | 2, refused in refine (header) | 2 |
| checkpoint 2, a blob's byte altered | 2 | 404,531,850 | 3, refused in refine (blobs) | 3 |
| checkpoint 2, timestamp altered | 2 | 404,591,891 | 13, wrong timestamp | 13 |
| checkpoint 2 | 2 | 404,591,891 | 1, accepted | 1 |
| checkpoint 3 | 1 | 262,393,344 | 1, accepted | 1 |

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
- **Not yet here:** the inbox (#50) and the proposer's signature and attestations (#54).

**A JAM detail found on the way.** A refused work item must still export its declared number
of segments. Otherwise the GP replaces its output with an error (`work_packages_and_reports.tex:215`)
and the reason for the refusal is lost, so refine exports zeros.
