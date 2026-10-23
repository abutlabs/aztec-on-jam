# Tracking Aztec's releases: what moved at the settlement boundary from v5 to v6 (#27)

The point of running Aztec unchanged above JAM (#28) is that a new Aztec release is a re-pin
plus whatever moved at the boundary. This file measures that for one release: **v5.2.1**
(8 October 2026, the last v5) against **v6.0.0** (22 October 2026). Sources: aztec-packages at
both tags (`l1-contracts/`, the protocol circuits' `constants.nr`). For the TypeScript,
v5.2.1's `yarn-project/` against aztec-node 1ac42dc. v6 moved the TypeScript out of
aztec-packages, and aztec-node's last commit before v6.0.0 is 1ac42dc (7 September; it has
no tags).

## The four boundary items

| Item | v5.2.1 | v6.0.0 | What a JAM port does |
|---|---|---|---|
| **The prover's proof format** (the epoch verifier `l1-contracts/script/deploy/HonkVerifier.sol`) | bb 5 layout: 29 subrelations | bb 6 layout: 31 subrelations (ROM-LogUp) and full-width challenges | Port the refine verifier. Done for v6 in #22: the diff of bb's own Solidity verifiers is two changes |
| **The epoch proof's shape** | `LOG_N = 24`; 111 public inputs + 8 pairing limbs | `LOG_N = 24`; 113 + 8 | Two more public inputs, `previous_inbox_rolling_hash` and `end_inbox_rolling_hash` (`ROOT_ROLLUP_PUBLIC_INPUTS_LENGTH`); refine's verifier takes any count, and the settlement service must supply them |
| **The Rollup's logic** (what the settlement service mirrors) | ProposeLib 399, EpochProofLib 479, RollupCore 632, Inbox 180, Outbox 214 lines | 547, 645, 730, 295, 214 | 182, 360, 182 and 273 lines changed; Outbox unchanged. The main change is a **streaming Inbox**: L1→L2 messages in a ring of rolling-hash buckets, eviction gated on proven consumption, and the epoch proof binding the inbox rolling hash. The settlement service's accumulate re-implements that |
| **The node's L1 interfaces** (what a JAM backend implements, `docs/aztec/BOUNDARY.md`) | `rollup.ts` 1,466, `sequencer-publisher.ts` 1,496 lines | 1,502, 1,719 | 80 and 379 lines changed. Same publisher methods; `RollupContract` gains `getAttesterCountAtTime`. `l2_block_source.ts` 11 lines, `epoch_cache.ts` 30 |

Unchanged across the release: blobs (6 per checkpoint, 4,096 fields each), at most 32
checkpoints an epoch, 8 pairing-point limbs, the 410-field recursive proof.

## What this says about tracking Aztec

- **The prover moves every major release.** bb 3 → 5 (#15) and 5 → 6 (#22) each changed the
  verifier's transcript or relations. Each port was small and mechanical, because bb
  generates a Solidity verifier per release and the diff is the spec. A port is days, not
  weeks, but it is never zero.
- **The Rollup's logic is where a release costs most.** v6 rewrote L1→L2 messaging. The
  settlement service mirrors the Rollup, so it follows. The epoch proof binds the new
  rolling hash, so a stale service would be caught by the verifier rather than silently
  diverging.
- **The node's TypeScript boundary barely moved.** It kept the same methods: the seams
  proposed in `BOUNDARY.md` would have survived this release untouched.

## Per release, check

1. `HonkVerifier.sol` against the last release: subrelations, challenge derivation, proof
   length. Port the refine verifier from bb's Solidity diff (`spikes/epoch-proof`, `#22`).
2. `ROOT_ROLLUP_PUBLIC_INPUTS_LENGTH` and the root rollup's public inputs: what the service
   must supply.
3. `ProposeLib`, `EpochProofLib`, `Inbox`, `Outbox`: the service's refine and accumulate.
4. The node's `RollupContract` and `SequencerPublisher` method sets: the JAM backend.
