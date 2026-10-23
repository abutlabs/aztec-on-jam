# Where Aztec's blob data lives on JAM (#24)

Aztec publishes each checkpoint's transaction data as EIP-4844 blobs: up to 6 per checkpoint,
131,072 bytes each. The epoch proof commits to them by KZG (`FinalBlobAccumulator`). PXEs and
Aztec nodes rebuild state from that data. On Ethereum, two guarantees come with a blob:
consensus checks it against its commitment, and nodes serve it for at least 4,096 epochs, about
18.2 days (consensus-specs `deneb/p2p-interface.md:69`, `MIN_EPOCHS_FOR_BLOB_SIDECARS_REQUESTS`,
at 3dad3d5). Longer-term copies are kept by Aztec's own nodes and archives.

## JAM's options (Gray Paper 0.8.0)

| Where | Kept for | Bound by | Cost | Verdict |
|---|---|---|---|---|
| The work-package (payload or extrinsic), the Audits DA | Until the report is audited: "need not be retained after the work-report is considered audited" (`assurance.tex:5`) | the package hash | Package bandwidth only | Too short alone: a PXE syncing a day later finds nothing |
| **Exported segments**, the Imports DA | **28 days**: "should be retained for 28 days and provided to any validator on request" (`assurance.tex:7`) | the report's segments root, erasure-coded across validators | 32 segments a blob (131,072 / 4,104 = 31.9); up to 3,072 exports a package, so 96 blobs (`definitions.tex:287,292`) | **The choice** |
| Service storage | For ever, while the deposit is held | state root | 1 unit of deposit per byte (`definitions.tex:259`): 131,072 a blob; 192 blobs an epoch is 25 MB of state per epoch | Rejected: unbounded state growth for data that only needs to outlive sync |
| Off chain only (Aztec nodes) | As long as they choose | a hash on chain | Nothing on JAM | Rejected: weaker than Ethereum gives Aztec today |

## Decision

Blob data enters JAM in the work-package that carries a checkpoint. Refine checks each blob
against the KZG commitment Aztec's circuits use (262,430,040 gas a blob, `spikes/kzg-gas`;
batching to come) and **exports the blob as segments**. The work-report's segments root
binds them, and validators keep their erasure-coded shards for 28 days. That is longer than
Ethereum's 18.2-day blob window, so Aztec nodes and archives get the same or a better window
to copy the data out, which is all Aztec relies on now.

**Why KZG, when JAM's own erasure coding already binds the data by hash:** Aztec's circuits
commit to blobs by KZG over BLS12-381, and the point is to run Aztec unchanged (#28). A
hash-committed variant would be cheaper in refine, but it means changing Aztec's blob circuits
and maintaining the fork, against the direction's purpose. It is recorded as an alternative
for #30, not taken.

## Open (#37)

- [ ] **The export path, measured on PolkaJam.** Refine exports a blob as 32 segments; a later
  package imports and re-checks one; the cost of export and import.
- [ ] **Batch verification** (c-kzg `verify_blob_kzg_proof_batch`): one pairing for many blobs.
  The 63M a blob for hashing and evaluation stays.
- [ ] **Package size** (#36): PolkaJam refuses packages near 256 KiB, so one blob per package
  today. The Gray Paper allows about 100.
