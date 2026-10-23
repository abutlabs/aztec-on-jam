# Implementation plan: Aztec's privacy runtime on a JAM tiny testnet

Goal: a shielded account (`zk.money`'s shield, send, unshield) running end to end on a
local six-validator JAM net, with Aztec's own Noir circuits, prover and wallet, and a
JAM service in place of Aztec's L1 contracts, sequencer and rollup proofs.

Everything below follows from one measurement, so it comes first.

## Step 0: measure the verifier (the gate)

Compile a `no_std` Rust UltraHonk verifier (start from `ultrahonk_soroban_verifier`,
Nethermind and Stellar's, BN254 and Keccak transcript) to the PVM in
`spikes/ultrahonk-gas/`, verify one `bb`-made proof of a small Noir circuit, and meter it
the way `zk-jam-service/spikes/groth16-gas` metered Groth16 (246,884,255 gas under GP
0.8.0, polkavm agreeing). Measure wall time on lasair too (448 Mgas/s since lasair#80).

| Verifier gas | Decision |
| --- | --- |
| under 1 billion | fits a tiny package as is; proceed |
| 1 to 5 billion | run the tiny nets with the full gas constants (G_R = 5 billion); the builder folds every package's proofs into one |
| over 5 billion | the builder wraps the folded UltraHonk proof in a Groth16 proof (247 million, fits anywhere); the UltraHonk verifier never runs on chain |

Also measure: a 32-deep Poseidon2 or Blake2 Merkle insert in refine (sets notes a
package) and the indexed-tree nullifier insert.

**Measured 2026-10-22** (#13): 302,381,955 gas plain, 312,279,376 gas zk, both accepted;
tampered proof and wrong public input rejected at about 12.8M. Verifier: zkVerify
`ultrahonk-no-std`, pure Rust. **Row chosen: under 1 billion.** Three verifications fit a
tiny package, sixteen a full one; no folding is forced. Price: the verifier parses bb 3.0.x
proofs, so Noir beta.16 and bb 3.0.3 are pinned (#15). Tree-insert gas: still to measure.

**Moved to bb 5.0.0, 2026-10-23 (#15):** 286,136,021 gas zk for the same circuit, and
167,281,732 for the transfer circuit in the service. Noir rc.5, bb 5.0.0, the verifier from
zkVerify's PR #9 pinned in our fork.

## Step 1: the service (`services/privacy/`)

Refine takes one work-item: `[folded proof][public inputs]`, where the public inputs are
what Aztec's tx-base and merge circuits already expose: old and new note-tree root, old
and new nullifier-tree root, the note hashes, the nullifiers, the encrypted logs' hashes.
Refine verifies the proof and emits `[ok][old roots][new roots][nullifiers][log hashes]`
in the 48 KB result. The tree arithmetic is inside the proof, as on Aztec; refine does not
rebuild the trees.

Accumulate, per item: the old roots must equal the stored roots, else the item is dropped;
every nullifier must be absent from storage, else dropped; then write the nullifiers,
the new roots, and the encrypted logs (from the package's extrinsic, by hash) at 1 per
byte. Deferred transfers in (shield): `[32-byte note commitment][asset]` in the memo,
appended to a pending-deposit list the next package must include. Deferred transfers out
(unshield): the proof's public inputs name a receiver service and amount; accumulate sends.

A second tiny service, `public-balance`, holds users' public JAM balance so there is
something to shield from on a dev chain. Both deploy with jamswap's `offchain/deploy.py`
through the Bootstrap service over JIP-2; the kernel verification keys are preimages the
service solicits.

Check (byte-exact): one shielded transfer on `lasair6` leaves exactly two new nullifiers
in storage, the root equals the one the builder computed, and a replay of the same
package is dropped.

## Step 2: the circuits (`aztec-packages` at v6.0.0)

**Done differently, 2026-10-23 (#7, #18):** a transfer circuit of our own in Noir
(`circuits/transfer`, Poseidon2, Aztec's note and nullifier shapes, depth-16 tree, deposit
claims, unshield), proven by `client/privacy.py`, running on lasair6 with `nets/demo.sh
check` passing. Aztec's kernels and Aztec.nr are not used yet; the plan below is the way
back to them and stays open.

Keep the private kernels (init, inner, reset, tail) and Aztec.nr unchanged. Add a tail
variant that drops the L1-to-L2 and L2-to-L1 message fields and anchors to the service's
block header (the JAM lookup anchor stands in for Aztec's archive). Keep tx-base-private
and tx-merge as the builder's folding circuits; drop block, checkpoint, root and blob
circuits. Export verification keys with `bb` into `aztec-circuits`, one image the builder
and the PXE mount. Changes stay under `noir-projects/`; Barretenberg is used as released.

## Step 3: the node pieces (`aztec-node` fork)

- **Indexer** (from the archiver): follows the chain over JIP-2, reads the service's
  storage and the packages' extrinsics, serves the PXE's node RPC: headers, tree
  witnesses, encrypted logs.
- **Builder** (from the sequencer client and prover node): collects proven transactions,
  runs tube, tx-base and tx-merge with `bb` to fold them, builds the work-package with the
  encrypted logs as extrinsics, submits with JIP-2 `submitWorkPackage` (or CE-133 to a
  lasair guarantor). One package a block for the tree.
- **PXE**: a node adapter pointing at the indexer; proving as today, in the browser.

Check: the builder's folded proof verifies in refine on the first try; the indexer's
witnesses match the service's roots at every height.

## Step 4: the wallet (`demo-wallet` fork)

Point it at the PXE and the indexer. Three buttons. Shield debits `public-balance`
and credits a note; send proves and submits; unshield burns and transfers out.

Check: a second browser profile receives a private send and can unshield it.

## Step 5: the nets (`nets/`)

| Net | Validators | Use |
| --- | --- | --- |
| jamswap `lasair6` | six lasair | development; refine on an interpreter (1 billion gas in about 2 s) |
| jamswap `pj6` | six PolkaJam | slot-paced throughput; the variation with no lasair |
| jamswap mixed, lasair `rehearsal-net.sh` | lasair among PolkaJam | the conformance view |

Tiny constants to override when the gate says so: G_R 1 billion (full 5 billion), lookup
anchor age 24 slots (full 14,400), preimage expunge 32 slots (full 19,200).

## Milestones

1. **Gate passed**: verifier gas and wall time measured, decision row chosen, written here.
2. **One transfer**: shield, send, unshield once on `lasair6`, storage checked byte for byte.
3. **Wallet**: `zk.money`'s three buttons work for two users in two browsers.
4. **Soak**: an hour on `pj6` at one package a slot, zero dropped roots, finality advancing.

## Risks, named

- Verifier gas (the gate). Unknown until step 0.
- Lasair's refine speed: fine to 1 billion gas; past that, guarantee on PolkaJam.
- Aztec moves monthly; the fork is pinned to v6.0.0 (the proving-system fix) and tracks by
  deliberate rebase, not by default.
- Encrypted logs at 1 per byte for ever; the indexer may archive them instead, trusted for
  discovery only.
- One package a block for the tree: the builder is a sequencer. Two builders means two
  trees means two crowds.
