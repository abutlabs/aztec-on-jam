# Is there a better privacy architecture for JAM than Aztec's? (#30)

Aodh's question (#35): *is there a drastically different privacy architecture that would work
better in JAM than Aztec (the compute challenge seems inherent with heavy/slow calcs occurring
on client hardware being the optimal architecture)?*

**Short answer: yes, one that uses what JAM has and Ethereum lacks.** That is many cores
re-executing in parallel, with refine's 5 billion gas a package. Users still prove on their
own devices, which Aodh is right to call inherent: the private inputs must never leave the
device. But JAM can then check every user's proof directly, across cores in parallel, with no
folding prover. It updates the shared tree in one refine a slot instead of in each proof. The
cost is a bounded number of tree insertions a slot, and user proofs small enough to check
cheaply: UltraHonk, as this demo's, not Aztec's own client proofs. **Measured** marks numbers from this
repository (PolkaJam unless stated); *reasoning* marks the rest.

## The candidates

| # | Architecture | How a transfer is checked | Throughput bound | Trust | Verdict |
|---|---|---|---|---|---|
| 1 | **Aztec's, with JAM as settlement** (#28) | Users prove; Aztec's provers fold an epoch; refine checks one epoch proof and its blobs | Aztec's own; JAM pays about 981M gas a checkpoint and 812M an epoch proof (**measured**, #23, #37, #39) | The proof, plus JAM's re-execution of the check | Runs Aztec as released (the PXE unchanged, #25); keeps Aztec's prover tier |
| 2 | **Each proof checked in refine, chained** (this demo) | Users prove against the exact current root; refine verifies; accumulate checks the root | One transfer a slot a tree: the soak (**measured**, #4) | Proof plus re-execution | Simple; one tree is slow |
| 3 | **Verify in parallel, tree in one refine, nullifiers in accumulate** (proposed here) | Users prove against any recent root (an anchor); many packages a slot, on many cores, verify proofs (**measured**: 170.3M gas each, linear, 29 a full package, `spikes/batch-verify`); accumulate rejects a nullifier already in storage (a storage read: our service's accumulate does a whole transfer in 103,089 gas, **measured**); one "tree" package a slot inserts the new notes in refine | Verification: 29 proofs a package, one package a core a slot. Insertion, one tree package a slot: a batch appended to an append-only depth-40 tree costs 438,966 gas a note (**measured**, #43), so 11,350 notes a full package, 5,675 two-note transfers a slot a tree, about 950 a second. Feeding one tree takes about 196 cores of verification | Proof plus re-execution; no prover tier | **Recommended for a JAM-native privacy service** |
| 4 | Sharded pools: many trees | As 2 or 3, one tree per core | Scales with cores | As above | Each tree is a smaller anonymity set; cross-tree transfers go through unshield and shield (*reasoning*) |
| 5 | Non-ZK: MPC committees, TEEs, FHE | Off-chain parties compute on hidden data | Off-chain | A committee's honesty, a hardware vendor, or FHE's cost | JAM's auditors re-execute refine, but cannot check that a TEE kept a secret or that an MPC committee did not collude; FHE in the PVM has no host calls and would be far heavier than proof checks (*reasoning*). Weaker than 1–3 for JAM |

## Why 3 suits JAM better than Aztec's model

- **Aztec folds because Ethereum is small.** Ethereum checks one proof an epoch, so Aztec's
  provers compress every transaction into it: a prover network and a proving delay. JAM has
  341 cores re-executing in parallel. At 170.3M gas a proof (**measured**), the chain can
  check 341 × 29 ≈ 9,900 users' proofs every 6 s. Folding there would be a cost with nothing
  to buy (*reasoning* from measured inputs). This holds for proofs the size of this demo's
  (UltraHonk, verified in about 170M gas). Aztec's client proofs (CHONK) would cost about 12B
  gas each in the PVM, 2.4 full packages (estimated from native verify times, `spikes/chonk`).
  Batched, one discharge serves every proof's deferred IPA, and each proof's reduction is about
  19 ms natively: about 1.4 to 1.7B gas a proof at batches of 64 to 512, about three a full
  package (`spikes/chonk-ipa`, #46, by the same ratio). That is a direct check of Aztec's own
  client proofs at small scale, without folding; at scale Aztec's folding (candidate 1) still
  costs far less a transaction. With the proposed curve calls about 15 ms a proof would stay in
  the PVM, half of it Poseidon2 (#62).
- **The ordering problem moves out of the proofs.** In 2, each proof names the exact current
  root, so proofs on one tree serialize at one a slot. In 3, a proof names any recent root
  (Zcash's anchors), and double spends are stopped by the nullifier set in accumulate, which
  is cheap storage, not a tree. Only note insertion is sequential, and it happens in one refine
  a slot, which has 5e9 gas. Appended as a batch, the notes share the tree's upper levels, so
  each costs about one hash: 438,966 gas, 11,350 notes a full package (**measured**, #43),
  against 290 if each rehashed its path. A Poseidon2 host call, left to a later JIP, would
  double it.
- **Accumulate stays small.** It checks nullifiers, records them, and accepts the tree
  package's new root if it started from the current one. #41 showed accumulate cannot afford
  even one insertion (17.1M gas against 10M), so 3 keeps all hashing in refine.

## What 3 costs, and what is unbuilt

- **Latency.** A new note can be spent once the tree package has inserted it: a slot or two
  after its transfer lands (*reasoning*).
- **The circuit changes.** Membership against a recent root rather than the current one, and
  no in-circuit nullifier tree (non-membership moves to accumulate's storage). That is simpler
  than this demo's circuit (*reasoning*).
- **The hand-off.** Refine cannot read state, so the tree package carries the slot's new notes
  in its payload and claims an old root, a new root and a hash of the notes. Accumulate accepts
  it only if the old root is current and the notes are those it queued from verified transfers,
  in order, which it tracks as a running hash (*reasoning*; not built).
- **Package size.** 29 proofs with keys and inputs are about 385 KB, past JAM-NP's 200 KiB
  message (#36), so above 15 a package they travel as extrinsics, as blobs do (#37).
- **Anonymity set.** One tree takes about 5,675 transfers a slot (#43), more than 196 cores can
  verify. So under today's specs one tree is bounded by verification, not insertion, and a
  JAM-native service needs no second tree until it uses a large share of JAM's cores.
- **The tree package's size.** 11,350 notes are 363 KB, so they travel as extrinsics, above
  about 6,300 a package (#36, #37).
- **Not built here.** A prototype of 3 is future work: the measured pieces are verification
  (`spikes/batch-verify`), nullifier storage (`services/privacy`), and the hash cost
  (`spikes/poseidon2-gas`, with the batch append, #43).

## Recommendation

- **To run Aztec as it is,** use 1 (JAM as settlement, #28): the most reuse, Aztec's prover
  tier kept.
- **For a JAM-native privacy service,** use 3: no prover network, users' proofs checked
  directly and in parallel by re-execution, and one tree's insertions done in refine once a
  slot.
- **Both benefit from JIP-8** (`docs/jip/jip-final.md`): an UltraHonk check at 52.4M gas
  instead of 181M, measured on lasair with the calls (#59), so 3 verifies 95 Aztec-sized proofs
  a full package instead of 27 (measured at 2^16: 104 fit, the 105th runs out of gas).
