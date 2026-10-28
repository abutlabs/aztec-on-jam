# Ten minutes with aztec-on-jam

Shielded payments as a JAM service: Aztec's model (notes, nullifiers, client-side proofs)
with JAM doing the checking. This page is for a JAM implementer who wants to see it run,
know what the chain actually does, and decide whether to believe it.

## What happens in one payment

1. **The sender proves, on their own machine** (a browser tab or the Python client), a Noir
   join-split: two notes in, two notes out, an optional unshield to a public account.
   Barretenberg makes an UltraHonk proof (zero-knowledge, Keccak transcript): 1.0 s in Chrome.
2. **A work-package** carries `[public inputs][proof][two sealed notes]` to the privacy
   service, submitted over JIP-2.
3. **Refine** verifies the proof on the PVM (a pure-Rust verifier inside the service blob,
   169 million gas, under a second), and checks the sealed notes are the ones the proof
   committed to. It reads no state.
4. **Accumulate** checks the proof started from the trees the chain holds (note root,
   nullifier root and size), writes the new roots, the nullifiers in order, the two new leaves
   and their sealed notes: 103,089 gas. Anything stale or replayed changes nothing.
5. **The receiver** scans the chain's sealed notes with their own key; the one that opens
   and hashes to its leaf is theirs. Nobody told them; nobody else can tell.

Value enters and leaves by **JAM deferred transfers**: a public-balance service debits the
sender and sends a transfer whose memo carries the note commitment (shield); the privacy
service sends one back whose memo names the receiving account (unshield).

## Run it

```sh
scripts/bootstrap.sh          # the pinned toolchain, the sibling checkouts, both services built
nets/demo.sh up               # PolkaJam's tiny testnet, both services deployed, indexer and wallet
nets/demo.sh check            # A shields, pays B, B finds it and unshields: 37 assertions
open "http://localhost:19710/?user=alice"        # and ?user=bob in another window: seeded dev accounts
open "http://localhost:19710/explorer.html"      # what the chain shows, and what a key reads in it
nets/demo.sh down
```

The default net is PolkaJam's own: `polkajam-testnet` from the v0.8.0 release runs six
validators and an RPC node on this machine, with no Docker. `NET=lasair6` and `NET=pj6` run the
same on Docker nets of six lasair or six PolkaJam validators; `NET=jb-pj6`, `jb-lasair6` and
`jb-mixed` on native nets that our jamboree fork launches, the last with three validators of
each client.
Every net is driven the same way: `jamboree rpc` deploys through the Bootstrap service, builds
and submits work-packages, and reads storage, from the chain spec alone.

| `NET` | Validators | Needs |
| --- | --- | --- |
| `polkajam` (the default) | 6 PolkaJam v0.8.0, launched by `polkajam-testnet` | the PolkaJam release |
| `lasair6`, `pj6` | 6 lasair, or 6 PolkaJam (a September nightly) | Docker, jamswap's compose |
| `jb-pj6`, `jb-lasair6`, `jb-mixed` | PolkaJam, lasair or three of each, launched by our jamboree fork | lasair's source for the last two |

The PolkaJam net's validators listen on UDP 41000 to 41005 and its JIP-2 endpoint on 42950. The
indexer serves on 19700 and the wallet on 19710. Logs and chain data stay in `out/<net>/`, and
`down` removes them. `JB_TELEMETRY=host:port` sends JIP-3 telemetry to a collector.

`nets/demo.sh shield`, `send` and `unshield` run one move each from the command line.
`node client/e2e/e2e.mjs` drives two browsers through the same moves and asserts every balance
the pages show. `node client/e2e/race.mjs` has two wallets send at the same moment: both proofs
start from the same trees, so the one that accumulates second is refused, and its page must
prove again from the new trees. `node client/e2e/pay-both.mjs` has Alice and Bob pay each other
in public at the same moment: each balance ends where it started, so a page knows its payment
landed by its nonce (`/seen/<nonce>`), not by its balance.

The wallet keeps its notes in the browser. When the net is restarted or switched, the chain no
longer holds those notes where the wallet found them, so the page forgets the old chain and
scans the new one. Reload an open page after `up`.

`check` prints every assertion: three note-root changes, each equal to the prover's and to
the root rebuilt from the chain's own leaves; the six nullifiers recorded and the nullifier
root chaining through the three proofs; B's client finding exactly its new note by scanning;
A down 100 publicly, B up 40; a replayed send changing nothing.

The browser wallet is self-custodial: its secret never leaves the tab; it publishes its
address (an encryption key and a spend key) on chain, scans for its notes, and proves every
transfer itself with noir_js and bb.js. The server behind it only serves files.

`up` also seeds the six standard JAM dev accounts of JIP-5: Alice, Bob, Carol, David, Eve and
Fergie. Each publishes its address and holds 1000 public. Their wallets derive from their
public seeds, so `?user=alice` opens Alice's wallet in any browser; any other name makes a
wallet whose secret never leaves that browser. `nets/demo.sh seed` seeds a running net again.

## Public and private, side by side

The wallet pays in the open as well as privately, and each move says what the chain shows of it:

| Move | Anyone can read on chain | Not on chain |
| --- | --- | --- |
| Pay publicly | the payer, the payee and the amount | |
| Shield | the account, the amount and a note commitment | who will own the note |
| Send privately | two nullifiers, two note commitments and two sealed notes | the sender, the recipient and the amount |
| Unshield | the amount and the receiving account | which notes paid for it |

The explorer at `http://localhost:19710/explorer.html` shows what the chain records: the two
services' storage, as any JAM node serves it over JIP-2. Search an address to see its public
payments, shields and payouts, with dev accounts named. A private transfer shows under an
address only when a public shield or payout ties it there; a private send from Alice to Bob
shows under neither of them.

Then choose a key to read with:

- **A dev account's keys, or a wallet in this browser.** The page marks the notes the key
  opens and their values, which of its notes each transfer spends, and its private balance.
- **A viewing key.** The wallet shows its own: the note-encryption secret and the spend public
  key, 64 bytes. It opens the notes sealed to it, with their values. It cannot spend them, and
  it cannot tell which were spent, because a nullifier needs the spend key.

Screenshots: [no key](images/12-explorer-public.png), [with Alice's
keys](images/13-explorer-alice-keys.png), [with Bob's viewing
key](images/14-explorer-bob-viewing-key.png), and [Alice's wallet](images/11-wallet-alice.png).
`node client/e2e/explorer.mjs` runs this story in browsers and asserts what each view shows;
`RECORD=1` films it into the README's demo GIF (on a fresh net; `FILM_NET=lasair-calls` names
lasair with the curve calls on the title card). `node client/e2e/film.mjs` makes the opening
crawl from [client/e2e/crawl.md](../client/e2e/crawl.md), no net needed.

The public-balance service keeps an ordered log of every public change with its slot. The
indexer serves it at `/history`, beside each private transfer's nullifiers, commitments, sealed
notes and the slot it landed in.

## What to read, in this order

| File | What it shows |
| --- | --- |
| `circuits/transfer/src/main.nr` | the whole privacy rule: membership, nullifiers via an indexed Merkle tree, balance, insertion, the sealed notes' binding |
| `services/privacy/src/lib.rs` | refine (verify) and accumulate (roots, nullifiers, leaves, transfers) |
| `client/privacy.py` | what a prover does: notes, both trees, the witness, the payload |
| `../demo-wallet/jam/lib/core.js` | the same, in a browser |
| `spikes/` | the measurements behind every number on the README |

## Where JAM fits, and where it strains

- **Refine is the right place for the proof.** 169M of a package's 5,000M gas: 29 proofs fit a
  full package, 5 a tiny one. Accumulate stays cheap at about 100k gas a transfer.
- **One tree takes one package a slot.** Each proof names the old root, so packages on one
  tree must land in order. The builder sends the next package once the previous is reported
  (a core holds one pending report), which sustains one a slot on PolkaJam: 600 of 600 in an
  hour, 13.4 s median from submission to state. More throughput means more trees (more cores)
  or folding many transfers into one proof.
- **Aztec's own client proofs fit refine only with a curve call.** Checked by Barretenberg's
  own verifier, compiled unchanged for the PVM, one costs 11.4B gas, more than two full packages.
  With the proposed Grumpkin MSM call (105, implemented only by lasair) it costs 3.46B, and on a
  six-validator lasair net such proofs were checked and finalized, VALID, and INVALID when
  tampered ([spikes/chonk-pvm](../spikes/chonk-pvm/RESULTS.md)). Ours costs 49M to 169M.
- **Clients matter.** Both interpreting, PolkaJam refines a privacy proof about 4.5 times faster
  than lasair. lasair's guarantors also wait for the core to be free before guaranteeing the next
  package, and one tree runs at about 0.38 of a slot on lasair against 1.0 on PolkaJam. *Two clients,
  side by side* below has the numbers.

## Numbers

| What | Value | Measured by |
| --- | --- | --- |
| One verification in refine (14 public inputs, zk, Keccak, bb 5.0.0) | **146,996,585 gas** on PolkaJam with field-wise curve hooks, under a second; 169,112,761 with the verifier's default hooks | `services/privacy/check.sh`, `spikes/jip8-lasair/pj-fallback.sh` ([#55](https://github.com/abutlabs/aztec-on-jam/issues/55)) |
| How many fit a package | 6 on tiny (1e9), 34 on full (5e9), by their gas | |
| The same verification with the proposed curve host calls ([docs/jip](jip/jip-final.md)), measured on lasair, the only client that implements them | **49,132,410 gas**: 20 on tiny, 101 on full. The same blob falls back to the hooks above where a net lacks the calls | `spikes/jip8-lasair/measure.sh` ([#59](https://github.com/abutlabs/aztec-on-jam/issues/59)) |
| Accumulate, one transfer (two nullifiers into the indexed tree, two leaves, two sealed notes, both roots) | 103,089 gas, 1% of a report's 10 million | lasair's in-process rig |
| A proof in the browser (Chrome, Apple M-series, 10 threads) | 1.0 s; loading the provers first takes 2.3 s, once | the wallet's own log; `client/e2e/e2e.mjs` |
| A proof with bb natively | 0.74 s (median of 600) | `client/soak.py` |
| A transfer, submitted to accumulated (PolkaJam, one a slot for an hour) | 13.4 s median, 15.8 s p99, 600 of 600 | `client/soak.py`, [soak-pj6.md](soak-pj6.md) |
| Note tree and nullifier tree depth | 40 each; depth 16 to 40 costs refine under 1% | `spikes/depth/measure.py` |
| Built for size instead of speed | 1.8x the gas: the PVM meters instructions, so the services build at opt-level 3 with LTO | [#13](https://github.com/abutlabs/aztec-on-jam/issues/13) |

## Two clients, side by side

The same service blob and the same proofs on two independent GP 0.8.0 clients. lasair is ours
(Linux images at `ghcr.io/abutlabs/lasair`; its source and tracker are not public yet),
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

Refine times are the clients' own (lasair's log, PolkaJam's JIP-3 `Refined` events;
`scripts/bench-refine-polkajam.sh` runs PolkaJam's in CI). A mixed net, three PolkaJam v0.8.0
and three lasair validators on GRANDPA, runs the whole demo (`check` in 1 min 36 s, a send in
13.1 s) with both clients finalizing. The lasair main column is one Apple M-series Mac, the six
validators native, blocks paced at JAM's 6 s.

Three things about lasair this work found and measured. A guarantor took packages for a core it
was about to leave and let them expire, and a package waited while its core held a report,
which held one tree to about 0.38 of a slot. Both are fixed in lasair main. And the cmov gas
rule lasair took with the v0.8.0 test vectors differs from PolkaJam's 2026-09-22 nightly by
2 gas on one accumulate, which forks a mixed net; PolkaJam v0.8.0 agrees with it.

## What is real and what is reduced

Real: the proofs (made by the sender, checked in refine; a tampered proof, an altered public
input or a swapped sealed note is refused), the state (written only by accumulate, from the
chain's own roots), the sealed notes and scanning, the public log the explorer reads,
self-custodial keys, deferred transfers between two services, and the networks (six validators
of an independent GP 0.8.0 client, or of PolkaJam, with GRANDPA finality, services deployed
through the Bootstrap service).

Reduced:

- **The circuit is ours, written to Aztec's shapes, not Aztec's kernel circuits.** Aztec v6's
  kernels compile with our toolchain and prove in 2 to 8 s
  ([spikes/chonk](../spikes/chonk/RESULTS.md)), and their proofs now check in refine, but at
  3.46B gas a proof with the proposed call 105 and 11.4B without, against 49M to 169M for ours
  ([spikes/chonk-pvm](../spikes/chonk-pvm/RESULTS.md)). Replacing our circuit with Aztec's own
  stack is the current work: [aztec/STACK.md](aztec/STACK.md),
  [#71](https://github.com/abutlabs/aztec-on-jam/issues/71).
- **One proof per package, no folding.** A tree takes one package a slot, in order; more
  throughput means more trees (cores) or folding many transfers into one proof.
- **Dev-chain money, unsigned.** The public-balance service has a faucet anyone may call, and
  its operations carry no signature. The private side is guarded by the proofs; the public side
  is a dev-chain ledger.
- **The dev accounts' secrets are public.** Anyone can open Alice's wallet; they exist to demo.
- **The keys live in the page.** JAM has no native user accounts: each service defines its own,
  and no browser wallet signs for these services.
- **Notes are found by trial decryption of every sealed note**, not Aztec's tagging.

## Aztec's own client proofs, checked on lasair

Separate from this demo, refine checks Aztec's own transaction proofs with Barretenberg's own
verifier, unchanged, compiled for the PVM. On six lasair validators with the proposed curve
calls: VALID three times at 3.46B gas, finalized; one octet flipped, INVALID. The build, the
gas and the limits: [spikes/chonk-pvm/RESULTS.md](../spikes/chonk-pvm/RESULTS.md); the proposal:
[jip/jip-final.md](jip/jip-final.md).
