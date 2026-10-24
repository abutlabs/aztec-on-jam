Aztec v6's L1 contract test fixtures, copied unchanged from aztec-packages v6.0.0 (0219f11,
`l1-contracts/test/fixtures/`, Apache-2.0). Aztec's node writes them from real checkpoints
(aztec-labs-eng/aztec-node v6.0.0, `sequencer-client/src/publisher/write_json.ts`). They hold
headers, header hashes and blob commitments, but not the blob data.
