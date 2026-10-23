# A post for the JAM implementers' channel

For Aodh to send; the loop never posts.

---

**Shielded payments as a JAM service, on six-validator tiny nets of lasair, PolkaJam, or both**

We built Aztec's privacy model as a JAM service and ran it end to end on local tiny nets: six
lasair validators, six PolkaJam validators, or three of each, with the same service blob.

- **Client-side proofs, checked in refine.** A Noir join-split, proved with Barretenberg
  (UltraHonk, zk, Keccak) in the sender's browser tab in about a second. A pure-Rust verifier
  inside the service blob checks it in refine for 169M gas: 5 fit a tiny package, 29 a full one.
- **Accumulate keeps the trees.** A note tree and an Aztec-style indexed nullifier tree, both
  depth 40, with non-membership proven in the circuit. Accumulate checks the old roots and size
  and records the rest for about 100k gas. Shield and unshield are deferred transfers between
  two services.
- **A self-custodial browser wallet.** Keys stay in the tab. Notes are found by scanning sealed
  notes on chain (X25519 + AES-256-GCM), and every proof is made in the page (noir_js + bb.js).
- **What it says about clients.** One tree takes one package a slot, in order. On PolkaJam an
  hour at that pace lost none of 600. On lasair, guarantors wait for a free core, so one tree
  runs at 0.385 of a slot; we fixed one expiry bug on the way. A mixed net needs both clients to
  agree on PVM gas to the unit: PolkaJam's 2026-09-22 nightly and the v0.8.0 vectors' cmov rule
  differ by 2 gas on one accumulate, which forks the net. PolkaJam v0.8.0 agrees.
- **Aztec's own kernels, measured.** They compile with Noir rc.3 and prove in 2 to 8 s, but
  verifying their proof costs about 70 times ours: around 12 billion gas, more than a full
  package. Running them on JAM means a builder that wraps each proof, which is Aztec's prover.
- **Testnet tooling.** `jamboree rpc`, in a fork of jamixir's jamboree: deploy a service through
  the Bootstrap service, build and submit work-packages, read state, all over JIP-2 from the
  chain spec alone. Offered upstream (jamixir/jamboree#49).

Repo, the demo in four commands, and every number with the command that measured it:
https://github.com/abutlabs/jam-privacy-service (start with docs/DEMO.md).
Questions and "that's wrong because…" very welcome.
