# JAM Rebel Wallet, on Aztec's libraries

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

The devnet: `tools/aztec-local/on-jam.sh --node --sequencer`. On first proof the tab downloads
about 20 MB of proving parameters (the CRS) from Aztec's CDN; everything else is served locally.
An Aztec release is a re-pin of `package.json`.
