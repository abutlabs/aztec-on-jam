# JAM Rebel Wallet, on Aztec's libraries

A private-payments wallet: send, shield (public → private), unshield (private → public), receive.
Beside it, a live view of who is doing what for each action: this tab proving, Aztec's node
checking the proof and building a block, the JAM sidecar handing the checkpoint to JAM, the JAM
validators verifying and finalizing it, and Aztec's prover proving the epoch, each with a timer
while it works. Every action keeps its own timeline, from the proof in the tab to the epoch proof
verified on JAM.

Our interface; everything cryptographic is Aztec's released code (`@aztec-labs/*` 6.0.0):
the PXE in the tab (SQLite on OPFS), Aztec's prover (bb.js, threaded, so the page is served
cross-origin isolated), Aztec's Token contract, Aztec's note tagging. Fees go through the
network's SponsoredFPC, so a fresh account needs no funds. Aztec's demo-wallet was the
reference for how the libraries fit; none of its code is copied (it carries no licence).

```sh
npm install && npm run build     # Node 22+
node serve.mjs                   # http://localhost:19720/?node=http://localhost:8080
node e2e.mjs                     # Alice and Bob in two browser contexts; needs the devnet up
```

The devnet: `tools/aztec-local/on-jam.sh`, which also serves the wallet. The live view reads
Aztec's node (`?node=`, default `http://localhost:8080`) and the JAM sidecar (`?jam=`, default
`http://localhost:8545`). On first proof the tab downloads about 20 MB of proving parameters (the
CRS) from Aztec's CDN; everything else is served locally. An Aztec release is a re-pin of
`package.json`.
