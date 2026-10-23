# jam-privacy-service

Shielded payments as a JAM service: Aztec's model (notes, nullifiers, client-side proofs) with
JAM doing the checking. A sender proves a private transfer in their own browser with a Noir
circuit and Barretenberg; `refine` verifies the UltraHonk proof on the PVM; `accumulate`
checks the proof started from the chain's trees and writes the new ones. Value enters and
leaves by JAM deferred transfers. It runs on local six-validator JAM tiny nets of lasair
validators, PolkaJam validators or both, with the same service blob on all of them.

![Two wallets, side by side: A shields, pays B, B finds the note and unshields](docs/images/demo.gif)

Ten minutes, for a JAM implementer: [docs/DEMO.md](docs/DEMO.md). Why this exists: [JAM
beside Aztec](https://abutlabs.github.io/the-gray-tutor/jam-beside-aztec). How it got here:
[docs/LEDGER.md](docs/LEDGER.md), every step with its command and output.

## Run it

You need Docker (running), Rust, Python 3.11+, Node 20+, git and Chrome. One script checks and
installs the rest at pinned versions ([docs/TOOLS.md](docs/TOOLS.md)), clones the sibling
checkouts (`../jamswap` for the Docker nets, `../demo-wallet` for the wallet page,
`../jamboree` for the native nets and `jamboree rpc`), and builds the circuits, both services
and the wallet's provers:

```sh
scripts/bootstrap.sh          # --check only reports
nets/demo.sh up               # six lasair validators; NET=pj6 for six PolkaJam validators
nets/demo.sh check            # A shields, pays B, B finds it by scanning and unshields; every value asserted
open "http://localhost:19710/?user=A"   # and ?user=B in another browser profile
nets/demo.sh down
```

Every stage takes a net: `NET=lasair6` (the default) and `NET=pj6` are Docker nets;
`NET=jb-pj6`, `jb-lasair6` and `jb-mixed` are native nets our jamboree fork launches
([nets/README.md](nets/README.md)). `nets/demo.sh shield`, `send` and `unshield` run one move
each from the command line.
`node client/e2e/e2e.mjs` drives two browsers through the same moves and asserts every
balance the pages show; `RECORD=1` records them side by side.

## How it works

| Step | Where | What |
| --- | --- | --- |
| Prove | the sender's browser (noir_js + bb.js) or `client/privacy.py` | a join-split: two notes in, two out, an optional unshield; membership in the note tree, both nullifiers inserted into an indexed nullifier tree (Aztec's shape), the two sealed notes bound by a hash |
| Submit | the indexer's relay | `[public inputs][proof][two sealed notes]` as one work-item, over JIP-2 |
| Refine | `services/privacy` on the PVM | verifies the proof (a pure-Rust UltraHonk verifier compiled into the blob) and that the sealed notes are the ones proved; reads no state |
| Accumulate | `services/privacy` | the proof's old note root, nullifier root and size must be the chain's; writes the new roots, the nullifiers in order, the two leaves and their sealed notes; a stale or replayed proof changes nothing |
| Receive | the receiver's browser | scans the sealed notes (X25519 + AES-256-GCM) with its own key; the one that opens and hashes to its leaf is theirs |

Shield: the public-balance service debits the sender and sends a deferred transfer whose memo
is the note commitment; a proof then claims it. Unshield: the proof names an account, and
accumulate sends a deferred transfer to it. Each wallet publishes a 64-byte address (an
encryption key and a spend key) on chain; its secret never leaves the browser.

## Numbers

| What | Value | Measured by |
| --- | --- | --- |
| One verification in refine (14 public inputs, zk, Keccak, bb 5.0.0) | **169,112,761 gas**, under a second | `services/privacy/check.sh` |
| How many fit a package | 5 on tiny (1e9), 29 on full (5e9) | |
| Accumulate, one transfer (two nullifiers into the indexed tree, two leaves, two sealed notes, both roots) | 103,089 gas, 1% of a report's 10 million | lasair's in-process rig |
| A proof in the browser (Chrome, Apple M-series, 10 threads) | 1.0 s; loading the provers first takes 2.3 s, once | the wallet's own log; `client/e2e/e2e.mjs` |
| A proof with bb natively | 0.66 s (median of 600) | `client/soak.py` |
| A transfer, submitted to accumulated (PolkaJam, one a slot for an hour) | 13.5 s median, 15.9 s p99, 600 of 600 | `client/soak.py`, [docs/soak-pj6.md](docs/soak-pj6.md) |
| Note tree and nullifier tree depth | 40 each; depth 16 to 40 costs refine under 1% | `spikes/depth/measure.py` |
| Built for size instead of speed | 1.8x the gas: the PVM meters instructions, so the services build at opt-level 3 with LTO | [#13](https://github.com/abutlabs/jam-privacy-service/issues/13) |

### lasair and PolkaJam, side by side

The same service blob and the same proofs on two independent GP 0.8.0 clients. lasair is
ours (Linux images at `ghcr.io/abutlabs/lasair`; its source and tracker are not public yet),
PolkaJam is Parity's.

| Measure | lasair | PolkaJam |
| --- | --- | --- |
| Refine one privacy proof (169M gas), interpreted, Linux aarch64 container | 0.94 s (2.1.3 image) | 0.207 s (v0.8.0) |
| The same, interpreted, macOS native | 0.65 s (main, a dev build) | 0.172 s (v0.8.0) |
| The same, PolkaJam's recompiler (x86-64 only; a GitHub runner) | no recompiler | 14 to 38 ms warm |
| An auditor re-executing a privacy report | 1.0 to 1.9 s | not in its telemetry |
| One tree at one package a slot (Docker nets, 20 min / 1 h) | 0.385 of a slot, median 21.2 s, 5 of 77 expired and resent | 1.0 of a slot, median 13.5 s, 0 of 600 |
| A private send in the wallet (Docker nets) | 27.5 s | 11.2 s |
| `nets/demo.sh check` (37 assertions) | 3 min 38 s | 1 min 51 s |

Refine times are the clients' own (lasair's log, PolkaJam's JIP-3 `Refined` events;
`scripts/bench-refine-polkajam.sh` runs PolkaJam's in CI). A mixed net, three PolkaJam v0.8.0
and three lasair validators on GRANDPA, runs the whole demo (`check` in 1 min 36 s, a send in
13.1 s) with both clients finalizing.

Three things about lasair this work found and measured. A guarantor took packages for a core it
was about to leave and let them expire: fixed in lasair main, expiries fell from 14 in 62 to 5 in
77. The rest wait for a free core across a rotation turn, and the same wait holds one tree to
0.385 of a slot: open. And the cmov gas rule lasair took with the v0.8.0 test vectors differs from
PolkaJam's 2026-09-22 nightly by 2 gas on one accumulate, which forks a mixed net; PolkaJam v0.8.0
agrees with it.

## What is real and what is reduced

Real: the proofs (made by the sender, checked in refine; a tampered proof, an altered public
input or a swapped sealed note is refused), the state (written only by accumulate, from
the chain's own roots), the sealed notes and scanning, self-custodial keys, deferred
transfers between two services, and the networks (six validators of an independent GP 0.8.0
client, or of PolkaJam, with GRANDPA finality, services deployed through the Bootstrap
service).

Reduced:

- **The circuit is ours, written to Aztec's shapes, not Aztec's kernel circuits.** Aztec v6's
  kernels compile with our toolchain and prove in 2 to 8 s, but verifying their 80 to 128 KB proof
  would cost about 12 billion gas in refine, more than a full package
  ([spikes/chonk](spikes/chonk/RESULTS.md)). Running them means a builder that wraps each proof
  (Aztec's prover) and Aztec's trees in accumulate: roadmap
  [#21](https://github.com/abutlabs/jam-privacy-service/issues/21).
- **One proof per package, no folding.** A tree takes one package a slot, in order; more
  throughput means more trees (cores) or folding many transfers into one proof.
- **Dev-chain money.** The public-balance service has a faucet anyone may call.
- **Notes are found by trial decryption of every sealed note**, not Aztec's tagging.

## Layout

| Path | What |
| --- | --- |
| `circuits/transfer/` | the Noir join-split (`hash_cli`, `path_cli`: the client's cross-checks) |
| `services/privacy/`, `services/public-balance/` | the two services; `crates/service-util/` what they share |
| `client/` | `privacy.py` (notes, trees, witnesses, payloads), `notecrypt.py`, `soak.py`, `wallet.py` (a static server), `e2e/` |
| `../demo-wallet/jam/` | the wallet page (branch `jam` of our demo-wallet fork) |
| `indexer/` | follows the chain over JIP-2 and serves it over HTTP; relays submissions |
| `nets/` | `demo.sh` (the stages), `net.sh` (lasair6, pj6), `jb.sh` (nets jamboree launches) |
| `scripts/` | `bootstrap.sh` (the pinned toolchain), `offline-proof.sh`, `fetch-crs.sh`, `bench-refine-polkajam.sh` |
| `spikes/` | the measurements behind the numbers |
| `docs/` | `DEMO.md`, `LEDGER.md` (every step with its evidence), `TOOLS.md`, the soak reports |

Apache-2.0.
