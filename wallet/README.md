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

## The explorer: the Imperial Ledger

A second page, on its own port (so its own origin and storage), opened from the wallet's
**Explorer ↗** link, laid out as a block explorer: search an address, a transaction hash or a
block; the home page shows the chain at a glance, the latest blocks and transactions, and what JAM
holds; each block, transaction and address has a page. What anyone sees: each Aztec block's transactions as
Aztec publishes them (note hashes, nullifiers, encrypted logs, public writes; no sender, recipient
or amount), each checkpoint's and epoch proof's work package on JAM, a token's public balances and
total supply read straight from public storage, and JAM's own chain over JIP-2.

Its private ledger is what viewing keys add. The wallet's **Receive → Show viewing keys** gives
the account's four privacy keys (nullifier-hiding, incoming-viewing, outgoing-viewing, tagging),
its public message-signing and fallback keys, and its senders: what Aztec's
`PXE.registerAccount` takes, and per Aztec not enough to spend. Pasted into the explorer, they go
into a PXE of its own, in memory, which finds that account's notes: its balance (spends counted),
the payments it received, and which public transactions those were. The account secret and the
signing key never leave the wallet; keys pass by copy and paste, never in a URL. Aztec's Token
tells only the recipient of a payment, so a sender's keys list no outgoing payments, though the
balance counts them.

```sh
npm install && npm run build     # Node 22+
node serve.mjs                   # http://localhost:19720/?node=http://localhost:8080
node serve.mjs 19730 explorer.html   # the explorer (on-jam.sh starts both)
node view-keys.mjs               # viewing keys in a fresh PXE: balances, history, no spending; needs the devnet up
node e2e.mjs                     # Alice and Bob in two browser contexts, then the explorer; needs the devnet up
node film/demo.mjs               # the demo film (docs/images/demo.gif), recorded from a live run
node film/film.mjs               # the intro film: the opening crawl, its words in film/crawl.md
```

The devnet: `tools/aztec-local/on-jam.sh`, which also serves the wallet and the explorer. The live
view reads Aztec's node (`?node=`, default `http://localhost:8080`) and the JAM sidecar (`?jam=`,
default `http://localhost:8545`); the explorer reads those and a JAM validator's JIP-2 RPC
(`?jip2=`, default `ws://localhost:42950`), which the wallet's link passes on. On first proof the tab downloads about 20 MB of proving parameters (the
CRS) from Aztec's CDN; everything else is served locally. An Aztec release is a re-pin of
`package.json`.
