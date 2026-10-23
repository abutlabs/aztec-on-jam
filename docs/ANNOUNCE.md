# A post for the JAM implementers' channel

Draft for Aodh to send (the loop never posts). Numbers are filled from the final
verification run before this line is removed.

---

**Shielded payments as a JAM service, running on a six-validator tiny testnet (lasair and PolkaJam)**

We built Aztec's privacy model as a JAM service and ran it end to end on local tiny nets, six
lasair validators or six PolkaJam validators, same service blob on both:

- **Client-side proofs, checked in refine.** A Noir join-split proved with Barretenberg
  (UltraHonk, zk, Keccak) on the sender's machine or in a browser tab; a pure-Rust verifier
  inside the service blob checks it in refine for about 169M gas (6 fit a tiny package,
  30 a full one).
- **Accumulate keeps the trees.** Note tree and Aztec-style indexed nullifier tree, both
  depth 40; non-membership proven in the circuit; accumulate checks the old roots and size
  and records the rest. Shield and unshield are JAM deferred transfers between two services.
- **A self-custodial browser wallet.** Keys stay in the tab, notes are found by scanning
  sealed notes on chain (X25519 + AES-256-GCM), proofs made in the page (noir_js + bb.js).
- **What it says about clients.** One tree takes one package a slot and packages must land
  in order; on PolkaJam an hour at one package a slot lost none; on lasair, packages can
  expire waiting for the guarantor's own authoring slot (lasair#93, with numbers).
- **Testnet tooling.** A fork of jamixir's jamboree with `jamboree rpc` (deploy a service
  through the Bootstrap service, build and submit work-packages, read state, over JIP-2 from
  the chain spec alone), offered upstream.

Repo, demo in four commands, and every number with its measurement:
https://github.com/abutlabs/jam-privacy-service (start with docs/DEMO.md).
Questions and "that's wrong because…" very welcome.
