# jam-privacy-service

Shielded payments as a JAM service: Aztec's model (notes, nullifiers, client-side proofs) with
JAM doing the checking. A sender proves a private transfer in their own browser with a Noir
circuit and Barretenberg; `refine` verifies the UltraHonk proof on the PVM; `accumulate`
checks the proof started from the chain's trees and writes the new ones. Value enters and
leaves by JAM deferred transfers. It runs on PolkaJam's own tiny testnet by default, and on
six-validator nets of lasair validators or of both clients, with the same service blob on all
of them.

![An opening crawl, a card of what is running and what is not, then Alice's and Bob's wallets side by side: Alice pays Bob in public, shields with a Noir proof made in her browser and sends Bob 40 privately; Bob finds the note and unshields; then the explorer, read with no key, with Alice's keys and with Bob's viewing key](docs/images/demo.gif)

**What the recording is.** After a short opening crawl, everything in it runs live; no step is
mocked. Aztec's proving stack, Noir 1.0.0-rc.5 and Barretenberg (bb.js 5.0.0, UltraHonk),
proves each private transfer in the sender's browser in about a second. The privacy service
verifies the proof in `refine` on PolkaJam v0.8.0's six-validator testnet (169M gas), and
`accumulate` writes Aztec-shaped state: notes, nullifiers, a depth-40 note tree and an indexed
nullifier tree. Notes are sealed to their receiver (X25519, AES-256-GCM) and found with the
receiver's own key; no key leaves the browser. The wallet is the JAM Rebel Wallet and the
explorer the Imperial Ledger: what the chain shows, and what a key reads in it.

**What it is not.** Not the Aztec network: no Aztec kernel circuits, PXE, sequencer or
Aztec.nr contracts; the circuit is ours, written to Aztec's shapes. Not real money: a dev chain
with a faucet and an unsigned public ledger, and the dev accounts' keys are public. Notes are
found by trying every one, not by Aztec's tagging, and each proof takes a work package of its
own, with no folding. [What is real and what is
reduced](#what-is-real-and-what-is-reduced) has the details. Separately, this repository runs
Aztec v6's own unchanged node and PXE settling on JAM, so far with Aztec's test-mode proofs and
a stand-in epoch circuit ([docs/LEDGER.md](docs/LEDGER.md), entries 62 to 69).

Ten minutes, for a JAM implementer: [docs/DEMO.md](docs/DEMO.md). Why this exists: [JAM
beside Aztec](https://abutlabs.github.io/the-gray-tutor/jam-beside-aztec). How it got here:
[docs/LEDGER.md](docs/LEDGER.md), every step with its command and output.

## Run it

The default net is PolkaJam's own tiny testnet. `polkajam-testnet`, from Parity's PolkaJam
v0.8.0 release, starts six validators and an RPC node on this machine with GRANDPA finality.
It needs no Docker and no lasair.

You need Rust, Python 3.11+, Node 20+, git and Chrome. Docker is needed only for the
`lasair6` and `pj6` nets. One script checks and installs the rest at pinned versions
([docs/TOOLS.md](docs/TOOLS.md)): the PolkaJam release, the sibling checkouts (`../jamswap`
for the blob builder and the Docker nets, `../demo-wallet` for the wallet page, `../jamboree`
for `jamboree rpc`), the circuits, both services and the wallet's provers.

```sh
scripts/bootstrap.sh          # --check only reports
nets/demo.sh up               # PolkaJam's tiny testnet, both services deployed, indexer and wallet: about 75 s
nets/demo.sh check            # A shields, pays B, B finds it by scanning and unshields; every value asserted
open "http://localhost:19710/?user=alice"        # and ?user=bob in another window: seeded dev accounts
open "http://localhost:19710/explorer.html"      # what the chain shows, and what a key reads in it
nets/demo.sh down
```

The PolkaJam net's validators listen on UDP 41000 to 41005 and its JIP-2 endpoint on
42950. The indexer serves on 19700 and the wallet on 19710. Its logs and chain data stay in
`out/polkajam/`, and `down` removes them. `JB_TELEMETRY=host:port nets/demo.sh up` sends
PolkaJam's JIP-3 telemetry to a collector.

Every stage takes a net in `NET` ([nets/README.md](nets/README.md)):

| `NET` | Validators | Needs |
| --- | --- | --- |
| `polkajam` (the default) | 6 PolkaJam v0.8.0, launched by `polkajam-testnet` | the PolkaJam release |
| `lasair6`, `pj6` | 6 lasair, or 6 PolkaJam (a September nightly) | Docker, jamswap's compose |
| `jb-pj6`, `jb-lasair6`, `jb-mixed` | PolkaJam, lasair or three of each, launched by our jamboree fork | lasair's source for the last two |

`nets/demo.sh shield`, `send` and `unshield` run one move each from the command line.
`node client/e2e/e2e.mjs` drives two browsers through the same moves and asserts every
balance the pages show. `node client/e2e/race.mjs` has
two wallets send at the same moment. Both proofs start from the same trees, so the one that
accumulates second is refused, and its page must prove again from the new trees.
`node client/e2e/pay-both.mjs` has Alice and Bob pay each other in public at the same moment:
each balance ends where it started, so a page knows its payment landed by its nonce
(`/seen/<nonce>`), not by its balance.

The wallet keeps its notes in the browser. When the net is restarted or switched, the chain no
longer holds those notes where the wallet found them, so the page forgets the old chain and
scans the new one. Reload an open page after `up`.

## Public and private, side by side

`up` seeds the six standard JAM dev accounts of JIP-5: Alice, Bob, Carol, David, Eve and
Fergie. Each publishes its address and holds 1000 public. Their wallets derive from their
public seeds, so `?user=alice` opens Alice's wallet in any browser. Any other name makes a
wallet whose secret never leaves that browser. `nets/demo.sh seed` seeds a running net again.

The wallet pays in the open as well as privately, and each move says what the chain shows of
it:

| Move | Anyone can read on chain | Not on chain |
| --- | --- | --- |
| Pay publicly | the payer, the payee and the amount | |
| Shield | the account, the amount and a note commitment | who will own the note |
| Send privately | two nullifiers, two note commitments and two sealed notes | the sender, the recipient and the amount |
| Unshield | the amount and the receiving account | which notes paid for it |

The explorer, the Imperial Ledger, at `http://localhost:19710/explorer.html` shows what the chain records: the two
services' storage, as any JAM node serves it over JIP-2. Search an address to see its public
payments, shields and payouts, with dev accounts named. A private transfer shows under an
address only when a public shield or payout ties it there. A private send from Alice to Bob
shows under neither of them.

Then choose a key to read with:

- **A dev account's keys, or a wallet in this browser.** The page marks the notes the key
  opens and their values, which of its notes each transfer spends, and its private balance.
- **A viewing key.** The wallet shows its own: the note-encryption secret and the spend public
  key, 64 bytes. It opens the notes sealed to it, with their values. It cannot spend them, and
  it cannot tell which were spent, because a nullifier needs the spend key.

Screenshots: [no key](docs/images/12-explorer-public.png), [with Alice's
keys](docs/images/13-explorer-alice-keys.png), [with Bob's viewing
key](docs/images/14-explorer-bob-viewing-key.png), and [Alice's
wallet](docs/images/11-wallet-alice.png). `node client/e2e/explorer.mjs` runs this story in
browsers and asserts what each view shows; `RECORD=1` films it into the GIF at the top of this
page, with the opening crawl and cards of `client/e2e/film.html` and a caption over each move
(`client/e2e/film.mjs`; on a fresh net: `nets/demo.sh down`, then `up`).

The public-balance service keeps an ordered log of every public change with its slot. The
indexer serves it at `/history`, beside each private transfer's nullifiers, commitments,
sealed notes and the slot it landed in. It finds that slot by walking back over JIP-2's
`parent` and reading the privacy service's transfer count at each block.

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
| A proof with bb natively | 0.74 s (median of 600) | `client/soak.py` |
| A transfer, submitted to accumulated (PolkaJam, one a slot for an hour) | 13.4 s median, 15.8 s p99, 600 of 600 | `client/soak.py`, [docs/soak-pj6.md](docs/soak-pj6.md) |
| Note tree and nullifier tree depth | 40 each; depth 16 to 40 costs refine under 1% | `spikes/depth/measure.py` |
| Built for size instead of speed | 1.8x the gas: the PVM meters instructions, so the services build at opt-level 3 with LTO | [#13](https://github.com/abutlabs/jam-privacy-service/issues/13) |

### lasair and PolkaJam, side by side

The same service blob and the same proofs on two independent GP 0.8.0 clients. lasair is
ours (Linux images at `ghcr.io/abutlabs/lasair`; its source and tracker are not public yet),
PolkaJam is Parity's.

| Measure | lasair, Docker (2.1.3 image) | lasair main, native (`jb-lasair6`) | PolkaJam |
| --- | --- | --- | --- |
| Refine one privacy proof (169M gas), interpreted, Linux aarch64 container | 0.94 s | | 0.207 s (v0.8.0) |
| The same, interpreted, macOS native | | 0.65 s | 0.172 s (v0.8.0) |
| The same, PolkaJam's recompiler (x86-64 only; a 4-vCPU GitHub runner, 30 refines) | no recompiler | no recompiler | 8 to 24 ms in 15; 0.41 to 0.56 s in 12, on the same nodes, cause not yet known |
| An auditor re-executing a privacy report | 1.0 to 1.9 s | | not in its telemetry |
| One tree, packages kept waiting on one core | 0.375 to 0.385 of a slot (20 min / 1 h), 5 of 77 and 7 of 75 expired and resent | a report in every block (a block every 6.7 s), 46 of 46 in 5 minutes, none expired or resent | 1.0 of a slot, 0 of 600 expired (1 h) |
| A shield, then a private send, in the wallet | 81.3 s, 17.5 s | 27.7 s, 15.2 s | 25.8 s, 11.2 s |
| `nets/demo.sh check` (37 assertions), and resends in it and the three stages | 3 min 09 s, 2 resent | 1 min 52 s to 2 min 00 s, none | 1 min 51 s, none |

On the default net, one run on an Apple M-series Mac: `check` passed with nothing resent,
and in the wallet a shield took 23.6 s and a private send 11.1 s.

Refine times are the clients' own (lasair's log, PolkaJam's JIP-3 `Refined` events;
`scripts/bench-refine-polkajam.sh` runs PolkaJam's in CI). A mixed net, three PolkaJam v0.8.0
and three lasair validators on GRANDPA, runs the whole demo (`check` in 1 min 36 s, a send in
13.1 s) with both clients finalizing.

Three things about lasair this work found and measured. A guarantor took packages for a core it
was about to leave and let them expire, and a package waited while its core held a report,
which held one tree to about 0.38 of a slot. Both are fixed in lasair main: guarantees go out
while the core is engaged and across an epoch turn, JIP-2 is served from the node's own state,
and the guarantee pool takes the package whose anchor expires first. And the cmov gas rule lasair
took with the v0.8.0 test vectors differs from PolkaJam's 2026-09-22 nightly by 2 gas on one
accumulate, which forks a mixed net; PolkaJam v0.8.0 agrees with it.

The lasair main column is one Apple M-series Mac, the six validators native, blocks paced at
JAM's 6 s (`lasair --block-time 6`); `scripts/verify-all.sh jb-lasair6` passes on it.

## What is real and what is reduced

Real: the proofs (made by the sender, checked in refine; a tampered proof, an altered public
input or a swapped sealed note is refused), the state (written only by accumulate, from
the chain's own roots), the sealed notes and scanning, the public log the explorer reads,
self-custodial keys, deferred
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
- **Dev-chain money, unsigned.** The public-balance service has a faucet anyone may call, and
  its operations carry no signature: anyone can pay, shield or burn from any public account.
  The private side is guarded by the proofs; the public side is a dev-chain ledger.
- **The dev accounts' secrets are public.** Anyone can open Alice's wallet; they exist to demo.
- **The keys live in the page.** JAM has no native user accounts: each service defines its
  own, and no browser wallet signs for these services.
- **Notes are found by trial decryption of every sealed note**, not Aztec's tagging.

## Layout

| Path | What |
| --- | --- |
| `circuits/transfer/` | the Noir join-split (`hash_cli`, `path_cli`: the client's cross-checks) |
| `services/privacy/`, `services/public-balance/` | the two services; `crates/service-util/` what they share |
| `client/` | `privacy.py` (notes, trees, witnesses, payloads), `notecrypt.py`, `soak.py`, `wallet.py` (a static server), `seed.mjs` (the dev accounts; `test-seed.mjs`), `queue_soak.py` (packages kept waiting on one core), `e2e/` (`e2e.mjs`, `race.mjs`, `pay-both.mjs`, `explorer.mjs`; `film.mjs` and `film.html` make the README's GIF) |
| `../demo-wallet/jam/` | the wallet page and the explorer (branch `jam` of our demo-wallet fork) |
| `indexer/` | follows the chain over JIP-2 and serves it over HTTP; relays submissions (`test_indexer.py`) |
| `nets/` | `demo.sh` (the stages), `net.sh` (the Docker nets lasair6 and pj6), `jb.sh` (the native nets: polkajam, and the nets jamboree launches) |
| `scripts/` | `bootstrap.sh` (the pinned toolchain; `bootstrap-clean.sh` runs it from nothing in a container), `offline-proof.sh`, `fetch-crs.sh`, `bench-refine-polkajam.sh` |
| `spikes/` | the measurements behind the numbers |
| `docs/` | `DEMO.md`, `LEDGER.md` (every step with its evidence), `TOOLS.md`, the soak reports |

Apache-2.0.
