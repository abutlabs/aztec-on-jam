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
  (UltraHonk, verified in about 170M gas). Aztec's client proofs (CHONK) cost more. Measured
  with bb's own verifier compiled for the PVM (`spikes/chonk-pvm`, #62), one proof's whole check
  is 11.0B gas, two full packages. Batched, the proofs share one IPA discharge, but that is a
  2^15 Grumpkin MSM of 9 to 10B gas, itself two packages. Each proof's reduction is 0.98B. With
  a Grumpkin MSM call (the proposed index 105, 1.93B at 2^15), one proof's whole check is 3.47B
  (**measured**, and finalized on a six-validator lasair net), and two proofs fit a package that
  discharges its own batch. That is a direct
  check of Aztec's own client proofs at small scale, without folding. At scale, Aztec's folding
  (candidate 1) still costs far less a transaction.
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

## Checking Aztec's own client proofs directly (#61, #62)

Can JAM check Aztec's client proofs (CHONK), with Aztec's circuits unchanged? This is measured
with Barretenberg 6.0.0's own verifier, unchanged, compiled for the PVM and run in refine
(`spikes/chonk-pvm`, #62). The native times come from `spikes/chonk-ipa`.
- **Per proof:** the reduction (everything but the IPA) is 0.98B gas, 18.4 ms natively. Folding
  the proof's deferred IPA into a batch adds about 0.1B (1.27 ms natively, at the MSM's measured
  rate).
- **Once a batch:** the IPA discharge, one 2^15-point Grumpkin MSM, is 9.0 to 9.8B gas, about
  100 ms natively.

A Grumpkin MSM host call (the proposed index 105) costs G_GK_MSM(32,768) = 1,926,735,634 gas, priced
by the proposal's rule on its reference machine (arkworks' p99 there is 289 ms). An earlier version
of this page used a floor of about 0.65B from bb's native time on this Mac; the priced call costs
three times that.

| Proofs in the batch | Gas a proof, no calls | With a Grumpkin MSM call |
| --- | --- | --- |
| 1 | 10.0 to 10.8B | 2.9B |
| 8 | 2.2 to 2.3B | 1.3B |
| 64 | 1.2B | 1.1B |
| 512 | 1.1B | 1.1B |

**Without call 105, a direct check does not fit one package.** The discharge alone is 9 to 10B.
It can be split, since an MSM's parts add up. Two or three packages each take a share, and
accumulate adds the results and compares them with the batch's claim. That works, but every batch
then spans several packages.

**With call 105, two proofs fit a package that discharges its own batch:** 2 × 1.08 + 1.93 = 4.1B.
A batch can be larger if packages that only reduce (four proofs each, 4.3B) hand their deferred
claims to one package that folds them and calls 105: (5.0 − 1.93) / 0.1 ≈ 30 claims. That is
about four proofs a package overall (*reasoning*; not built). The call matters at every batch
size, because without it no single package can finish a batch.

**Measured with the call, on a live net.** bb's Grumpkin MSM, linked to call 105, makes one
proof's whole check 3.47B gas (from 11.36B in the same build): the reduction 0.81B, the CRS
points' parsing and the IPA's MSM through the call. On six lasair validators with the full
spec's refine gas, the proof was checked VALID three times and once tampered INVALID, each
verdict accumulated and finalized (`spikes/chonk-pvm/on-lasair.sh`). That is one proof, not yet
a batch.

**What is left is the reduction**, 0.98B a proof. Natively (#62 step 1) it splits into:
- Poseidon2 transcript hashing, 43%;
- BN254 work, 25% (calls 101 and 102 take it);
- sumcheck and field arithmetic, 32%.

By that split, the proposed curve calls with a Poseidon2 call would leave about a third of the
reduction in the PVM. That is an estimate: the split is native time, and the PVM's cost differs by
kind of work. The reduction costs 53M gas a native millisecond and the MSM 87 to 94M.

**What would change this:** routing bb's BN254 work to calls 101 and 102 in the PVM build, and
measuring the batch discharge itself rather than one proof's IPA.
