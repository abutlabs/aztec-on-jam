# Ledger

Appended by every iteration of the build loop (`docs/RALPH.md`). Newest at the bottom.

## 2026-10-22 0 — scaffold
Did: repositories created and forked; plan written; service crate's entry points and item
layout; the scaffold builds to a 16 KB blob with `jam-pvm-build 0.1.28`.
Evidence: `jam-pvm-build -m service` → `Written JAM-PVM blob for privacy-service … 15846 bytes`
Next: step 0 of the plan, the verifier measurement, in `spikes/ultrahonk-gas/`. Install
`aztec-up` for `bb` and `aztec-nargo` first; record versions in `docs/TOOLS.md`.
Blockers: none known. Docker Desktop is installed (/Applications/Docker.app) but its daemon was not running at 2026-10-22 22:50; `open -a Docker` and wait for `docker info` before any compose step.

## 2026-10-22 1 — step 0, the verifier measured (gate passed)
Did: installed Noir 1.0.0-beta.16 and bb 3.0.3 (rc.5 + bb 5.0.0 and beta.16's nightly bb both
fail the verifier's parser); chose zkVerify `ultrahonk-no-std` (pure Rust, no_std) after
ruling out the Soroban one (host BN254) and co-noir's (std); built the spike with
jamswap's `jam080-build` (the crates.io `jam-pvm-build` targets GP 0.7.2, so the service
crate now points at jamswap/tools/jam080); metered one verification in refine on lasair
with negative controls; replayed it in polkavm. Decision row: **under 1 billion**.
Evidence: `spikes/ultrahonk-gas/measure.sh` →
  `| plain (evm-no-zk) | 302381955 | 01 |`, `| zk (evm) | 312279376 | 01 |`,
  `| zk, one proof byte flipped | 12801522 | 00 |`, `| zk, wrong public input | 12796090 | 00 |`;
  `scripts/pvm-refine-differential.sh out/ultrahonk-gas-service.jam 01` →
  `OK refine rep.0 steps=81684163/81684164 gas left=4687720624` … `2 invocations, 0 divergent`
  (5e9 − 4687720624 = 312279376, lasair's figure exactly); blob 263,803 bytes; ~0.9 s wall.
Issues: #13 (benchmark, table), #11 (research, decision), #15 (decision: bb 3.0.3 layout), #12 (tools).
Next: step 1, the service (#6): refine takes `[proof][public inputs]` from the payload and
verifies with the vk embedded (later: vk from a preimage); accumulate as scaffolded; a
`public-balance` helper; deploy both on lasair6 with jamswap's deploy.py and get one
work-item accumulated. First measure a 32-deep Merkle insert's gas in refine (PLAN step 0's
second line), since it sets notes a package.
Blockers: none. Note for step 2: Aztec v6.0.0's bb is newer than 3.0.3 (#15).

## 2026-10-22 2 — step 1, part one: the service verifies a real proof and writes state
Did: a stub transfer circuit (`circuits/transfer_stub`, Noir beta.16, pedersen binding; the
public-input shape only, #16) proven with bb 3.0.3 (zk, Keccak; proof 8,640 bytes, 8 public
inputs); the service's refine now takes `[pubs_len u16][pubs][proof]`, verifies against the
embedded vk and outputs `[1][pubs]`; accumulate checks the stored roots, refuses spent
nullifiers, writes nullifiers, note hashes, new roots and a transfer count. Checked on
lasair's in-process rig with `services/privacy/check.sh`. Skipped the ledger's Merkle-insert
measurement: the plan keeps the tree arithmetic inside the proof, so refine never hashes a
tree; nothing to size.
Evidence: `services/privacy/check.sh` → `PASS: 12 assertions`; refine `gas used : 183558822
steps: 59756538`, accumulate `gas used : 83827`; storage after:
`"root:note" = 0x…03e9`, `"root:null" = 0x…07d2`, `0x6e663a1e95b928… = 0x01` (nf: + nullifier 0),
`0x6e663a…002a = 0x01`, `0x6e683a…01f5 = 0x01`, `0x6e683a…01f6 = 0x01`, `"transfers" = 0x0100000000000000`;
tampered proof → `verdict : INVALID`, `no storage change`; altered public input → the same.
Blob 1,197,205 bytes (the spike's was 263,803; same verifier — look into it, limit is 4 MB).
Issues: #6 (progress), #13 (second gas point), #16 (decision: stub circuit).
Next: step 1, part two: the `public-balance` helper service; both services deployed on
jamswap's lasair6 with `offchain/deploy.py`; one work-item through JIP-2 `submitWorkPackage`
(jamswap `offchain/chain.py`) and the same storage read back over JIP-2; then a second
package with the same proof must be dropped (double spend), which the in-process rig
cannot show.
Blockers: none.

## 2026-10-22 3 — step 1, part two: both services live on lasair6, double spend dropped
Did: a shared `crates/service-util` (keys, little-endian reads); `services/public-balance`
(`[op][account 32][amount u64]`, mint and burn, `bal:<account>`); `nets/lasair6.sh`
(up, build, deploy, ctl, down) and `nets/jamctl.py` (submit, read, head over JIP-2, run in
a python container on the compose network because the reader publishes no host port);
started jamswap's lasair6 (six lasair 2.1.3 validators, reader, builder; services at
genesis: 0 Bootstrap, 100 jamswap); deployed both services through the Bootstrap service
with jamswap's deploy.py (the 1.2 MB privacy blob as a preimage); sent the stub-transfer
proof, read the state, sent the same proof again, minted on public-balance.
Evidence: `deploy.py` → `service 1 ready (created #19, balance 1000012094) SERVICE_ID=1`
(public-balance), `service 2 ready (created #32, balance 1001196483) SERVICE_ID=2` (privacy).
`nets/lasair6.sh ctl submit 2 <8898-byte payload>` → `Reportable` at slot 39, `Reported`
slot 41, `Ready` slot 42; reads: `root:note = …03e9`, `root:null = …07d2`,
`transfers = 0100000000000000`, `nf:<nullifier 0> = 01`. Same payload again → `Ready` at
slot 46 (refine accepts a valid proof), then `root:note = …03e9`, `transfers = 0100000000000000`
unchanged: accumulate dropped the double spend. `ctl submit 1 00<a×32><100 LE>` → `Ready`
slot 52; `bal:<account> = 6400000000000000` (100).
Issues: #6 (step 1 done, closed), #2 (milestone 2 progress: send works; shield and unshield
as transfers remain), #10 (nets: lasair6 path exists).
Next: step 1, part three (#6 follow-up, under #2): shield and unshield as deferred
transfers: public-balance burns and sends `transfer(privacy, amount, memo = note
commitment ++ asset)`; the privacy service's accumulate receives the transfer item and
records the pending deposit; unshield the other way from a proof's public inputs. Needs
the `transfer` host call from jam-pvm-common's accumulate module and a public-input slot
naming a receiver and amount in the stub circuit. Then `nets/demo.sh check`'s first
version against lasair6.
Blockers: none. The net is left running.

## 2026-10-22 4 — step 1, part three: shield and unshield by deferred transfer; demo check v1
Did: the stub circuit gained three public inputs (unshield amount, receiver, destination
service; 11 in all, 352 bytes), re-proven; `service-util::be_u64`; public-balance op 2
*shield* `[02][account][amount][dest u32][commitment]` burns and `transfer(dest, amount,
100_000, memo = commitment ++ "jam")`, and credits a receiver named in an incoming
transfer's memo; the privacy service records incoming transfers as `dep:<commitment>` and
`shielded`, and after a valid proof with a non-zero amount sends `transfer(dest, amount,
memo = receiver ++ "unshield")` and counts `unshielded`. Redeployed both on the running
lasair6 (public-balance = 3, privacy = 4; the proof's destination field is 3). Wrote
`nets/demo.sh` (up, check, down) and ran `check`: mint, shield, send + unshield, replay.
Evidence: `nets/demo.sh check` → `PASS` in 1 min 54 s; the reads over JIP-2:
`bal:A = 6400000000000000` after the mint; after the shield `bal:A = 0000000000000000`,
`dep:C = 6400000000000000`, `shielded = 6400000000000000`; after the proof `root:note = …03e9`,
`root:null = …07d2`, `nf:<nullifier 0> = 01`, `nf:<nullifier 1> = 01`, `transfers = 0100…`,
`unshielded = 2800000000000000`, `bal:B = 2800000000000000` (40 arrived on the public side);
after the replay (`Ready`, so refine accepted the valid proof) `transfers`, `unshielded` and
`bal:B` unchanged. `services/privacy/check.sh` still `PASS: 12 assertions`.
Issues: #2 (milestone 2 closed, with the stub caveat), #10 (nets progress), #16.
Next: step 2 (#7): replace the stub with a real transfer circuit in Noir, Poseidon2 from
noir-lang/poseidon, keeping the 11 public inputs in order: Merkle membership of the two
spent notes against the old note root, nullifier = H(note, secret), the two new notes
balance the spent ones minus the unshield amount, the new note root is the old tree plus
the two note hashes, the new nullifier root is the old plus the two nullifiers. Then a
prover-side script (`client/`) that builds the witness from a local note set so A can
really pay B, and demo.sh check reads B's private balance from the notes.
Blockers: none. The net is left running (services 3 and 4).

## 2026-10-23 5 — step 2: the real transfer circuit, proven and paid on lasair6
Did: `circuits/transfer` (Noir beta.16, Poseidon2 from noir-lang/poseidon v0.2.6; v0.3+
needs a newer compiler): join-split of two notes in and two out, Merkle membership of spent
notes in a depth-16 append-only tree, nullifier = H(note hash, owner sk), owner pk = H(sk),
note hash = H(pk, value, blinding), balance in = out + unshield, the two new leaves inserted
into an empty slot proven under the old root, a deposit claim (input 0 may be a shield
deposit: its nullifier is the commitment and the proof returns the amount), the nullifier
"root" a running hash. Twelve public inputs: two params, ten returned by the circuit, so
the prover never states a root it did not compute. `circuits/hash_cli` is the client's
hash oracle (the circuit's own Poseidon2). `client/privacy.py` (stdlib Python driving
nargo and bb): keys from names, deposit notes, the tree with zero hashes, witnesses,
proofs, payloads, and `sync` from the chain's `leaves` and `leaf:<i>` keys, which the
service now writes. The service checks `dep:<nullifier 0>` holds the proof's deposit
amount and removes it. The stub circuit is deleted (#16). Dummy inputs take the sender's
key and a fresh blinding so their nullifiers never repeat (found when tx2 reused tx1's).
Evidence: `client/privacy.py transfer …` builds a proof in about 2.7 s on this laptop.
`services/privacy/check.sh` → `PASS: 6 assertions`; refine of the real proof
`gas used : 182188752 steps: 59325612`. `nets/demo.sh check` on lasair6 (public-balance 3,
privacy 5) → `PASS` in 2 min 05 s, twice in a row on the same chain (the second run
synced 2 leaves and inserted at 2 and 3): mint → `bal:A = 100`; shield → `bal:A = 0`,
`dep:C = 100`; tx1 (claim, pay B 40, keep 60) → `dep:C = (none)`, `root:note` and
`root:null` equal the proof's returned roots, both nullifiers and both leaves recorded,
`transfers` +1; tx2 (B spends 40, unshields 40 to account 2 on service 3) → root moves,
`unshielded` +40, `bal:B` +40; replay of tx2 → `transfers` and `bal:B` unchanged.
Client view after: A holds 60 privately; B's notes are spent.
Issues: #7 (closed), #16 (superseded, closed), #18 (decision: the circuit's
simplifications), #13 (run 3), #10, #17 (Aodh: testnet plumbing, jamboree assessed).
Next: step 3 (#8): the pieces around the chain. Not the aztec-node fork yet: first make
`nets/demo.sh up` work from a clean machine state in one command (start, deploy, record
ids) and time it, then an indexer (a long-running reader of `leaves`, `leaf:<i>`,
`nf:`, roots over JIP-2 serving HTTP to the wallet), then the wallet (#9) as a small web
page over the client (the demo-wallet fork needs the Aztec PXE, which this circuit does not
speak; decision to record). Also: the privacy blob is 1.2 MB against the spike's 264 KB
with the same verifier (look into it), and the refine gas of the real proof vs the spike.
Blockers: none. The net is left running.

## 2026-10-23 6 — step 3, part one: up from a clean machine in one command
Did: `nets/demo.sh up` now starts the net, waits for the JIP-2 reader, clears old ids and
client state, builds and deploys both services and prints their ids; `nets/demo.sh down`
wipes the chain. Fixed `nets/lasair6.sh` to mount the lasair6 compose's own volume
(`jamswap_lasair6_shared`; it had been reading `spec.json` from another net's stale volume,
which worked only because the dev genesis shares its authorizer). Ran down, up, check.
Evidence: `nets/demo.sh down` → containers gone in 11 s. `time nets/demo.sh up` →
`services: public-balance 1, privacy 2`, **1 min 40 s** (images cached; a first-ever run
also builds `jamswap-pj-mixed:local`, which downloads a PolkaJam nightly). `time
nets/demo.sh check` on the fresh chain → `chain before: 0 leaves, 0 transfers …`, 20 `ok`
lines, `PASS`, **2 min 08 s**. Leaves 0 and 1 are B's 40 and A's 60; after tx2 `bal:B = 40`,
`unshielded = 40`, `shielded = 100`; the replay changed nothing.
Issues: #10 (nets), #8 (step 3 opened for real).
Next: step 3, part two: an indexer, one long-running process that follows the chain over
JIP-2 and serves HTTP: `/state` (roots, leaves, counts), `/leaves`, `/nullifier/<hex>`,
`/balance/<account>`; the client's `sync` reads it instead of looping over `ctl read`,
and the wallet (#9) reads it too. Python, stdlib, in the same container pattern as jamctl,
published on a host port. Then the wallet page.
Blockers: none. The net is left running (services 1 and 2).

## 2026-10-23 7 — step 3, part two: the indexer, and a lost package explained
Did: `indexer/indexer.py` (stdlib Python on jamswap's offchain modules, in a container on
the net, HTTP on localhost:19700): follows the privacy service over JIP-2 every 3 s and
serves `/state` (roots, leaves, counts), `/leaves`, `/nullifier/<hex>`, `/deposit/<hex>`,
`/balance/<account>`, `/head`, `POST /submit/<sid>`, `/status`. `nets/lasair6.sh indexer`
starts it; `nets/demo.sh up` starts it after the deploy; the client's `sync` takes
`/state` in one request (falls back to key-by-key reads). Found a package that went
`Ready` on JIP-2 but never accumulated: lm0 refined and co-signed it twice, then
`EXPIRED (anchor_too_old)`; 63 availability-retention drops against 11 accumulations in
20 minutes on the six nodes. Filed abutlabs/lasair#93. `nets/demo.sh` now has
`submit_until`: a payload whose state change is not seen in 36 s is sent again (safe:
accumulate is a no-op for a payload it already applied). Aodh: fork jamboree instead of a
new testnet repo; forked to abutlabs/jamboree, submodule added, plan on #17 revised.
Evidence: `nets/lasair6.sh indexer` → `indexer up: slot 67 leaves 4 transfers 2`;
`curl localhost:19700/state` → `{'slot': 67, 'leaf_count': 4, 'transfers': 2, 'shielded': 100,
'unshielded': 40, 'note_root': '15f7…'}`; `/balance/<B>` → `{"balance": 40}`;
`/nullifier/<42>` → `{"spent": false}`. Lost package: `lm0 [ce133] package 0x03ae5e7145c02d61..
EXPIRED (anchor_too_old)` after `refined … 182164641 gas`; resubmitted by hand → `transfers`
3 → 4. `nets/demo.sh check` (third run on the same chain, synced 8 leaves from the
indexer) → `PASS` in 1 min 42 s, no resend needed.
Issues: #8 (part two), #10, #17 (jamboree plan), lasair#93.
Next: step 4 (#9): the wallet. Not the demo-wallet fork (it speaks the Aztec PXE, which
this circuit does not; decision to record on #9): a small web page served next to the
indexer, over `client/privacy.py` through a local HTTP shim (`client/wallet.py`): shows
the public balance and the private notes for A and B, three buttons (shield, send,
unshield) that build the payload and POST it to the indexer's `/submit`, and the chain's
state live from `/state`. Screenshots to `docs/images/` for the two-browser check (#3).
Blockers: none. The net is left running.

## 2026-10-23 8 — step 4: the wallet, two users in two browsers (milestone 3)
Did: `client/privacy.py` refactored into callable functions (`deposit`, `transfer`, `sync`,
`unspent`, `select_inputs`; a `ClientError` instead of exits; the CLI prints the same lines).
`client/wallet.py`: serves the page and `/api/{state,mint,shield,send,unshield}`, proves on
the host, submits through the indexer, waits until the chain shows the change, resends
under lasair#93, and rolls the note state back if a proof never lands (a shield's deposit
note survives a failed claim). The page: `jam/` on the `jam` branch of abutlabs/demo-wallet,
plain HTML/CSS/JS in the demo-wallet's palette (decision #19). `nets/demo.sh wallet` starts
the backend; `up` starts it too. Found and fixed a real bug: a resent mint or shield was
not idempotent; public-balance now applies each nonce once (`seen:<nonce>`; a mint carries
32 random bytes, a shield its commitment). `client/e2e/e2e.mjs`: puppeteer-core on the
installed Chrome, user A and user B in two isolated browser contexts.
Evidence: curl smoke test → `mint … not on chain after 36 s … (resend 1) … on chain after
6 s`; `shield … on chain after 19 s`, `claim: proof of 8642 bytes in 2.4 s … on chain after
20 s`. Nonce on chain: one mint payload sent twice → `bal:C = 6400000000000000` both
times. `node client/e2e/e2e.mjs` → `PASS: 5 assertions, 6 screenshots` in 3 min 10 s:
A mint 100 (27.5 s), A shield 100 (68.7 s), A send 40 to B (17 s), B sees private 40,
B unshield 40 (73.5 s, one resend), B public 40, private 0. Screenshots
`docs/images/01-a-start.png` … `06-b-unshielded.png`. `nets/demo.sh check` after the
format change → `PASS` in 3 min 19 s (one mint resend, applied once).
Issues: #9 and #3 closed, #19 (decision), #10, lasair#93 (seen live in both runs).
Next: step 5, the README's demo: stage commands `shield`, `send`, `unshield` in
`nets/demo.sh` (one word each, defaults A→B, calling the wallet API), a *Demo* section and a
*What is real and what is reduced* section in README.md, then the full verification run
from `down` → `up` → `check` → e2e on a clean chain, copied into this ledger.
Blockers: none. Net, indexer and wallet left running.

## 2026-10-23 9 — step 5: stage commands, README, and the verification run (demo ready)
Did: `nets/demo.sh check` restructured to the demo-ready wording: three proofs (the claim
of A's shield, A's send of 40 to B, B's unshield of 40), the root asserted three times,
each equal to the prover's, and equal to the root the client rebuilds from the chain's own
leaves; the six nullifiers and the nullifier root chained tx1 → tx2 → tx3; A at −100 from
the shield, B at +40; a replay of the *send* dropped. Stage commands `shield`, `send`,
`unshield` over the wallet API (defaults A 100, A→B 40, B 40; the faucet tops A up).
`up` also clears the wallet's note state. README: *Demo* (six one-word stages), *Numbers*,
*What is real and what is reduced* (each reduction linked to its issue). A complete send's
accumulate measured on the rig: 103,167 gas. The first stage-command run failed on my
output formatter (an f-string with backslashes, Python 3.11); the faucet mint had landed
and the wallet refused the send and unshield (409, nothing private yet), so nothing was
damaged; fixed and re-run below.

### Verification run, started 2026-10-23T04:29:35Z, copied from the terminal

`nets/demo.sh down` → no containers, no wallet process:
```
$ docker ps --format '{{.Names}}' | grep -cE "jamswap|jam-privacy"
0
$ pgrep -f client/wallet.py || echo "no wallet process"
no wallet process
```
`nets/demo.sh up` (from that clean state; Docker images cached) → six validators, reader,
builder, both services, indexer, wallet:
```
SERVICE_ID=1
deploy: privacy-service 0.0.1 (1203645 bytes, hash 546955055ca5265e..)
deploy: Bootstrap service 0: jam-bootstrap-service 0.1.29
deploy: CreateService(id 2, 1203645 bytes, endowment 1000000000) to service 0: package 57bc4ed0e4bb207d.. on core 0, anchor #7
deploy: service 2 created
deploy: submitPreimage(2, 1203645 bytes) at #12
deploy: service 2 ready (created #12, balance 1001203846)
SERVICE_ID=2
services: public-balance 1, privacy 2
indexer up: slot 15 leaves 0 transfers 0
wallet: http://localhost:19710/?user=A   (and ?user=B)   log: out/wallet.log
nets/demo.sh up  1.52s user 0.67s system 2% cpu 1:23.55 total
```
```
$ docker ps --format '{{.Names}}'
jam-privacy-indexer jamswap-builder-1 jamswap-lm0-1 jamswap-lm1-1 jamswap-lm2-1 jamswap-lm3-1 jamswap-lm4-1 jamswap-lm5-1 jamswap-reader-1
```
`nets/demo.sh check` → exit 0, 34 `ok`, 0 `FAIL`:
```
public-balance = service 1, privacy = service 2
== the client learns the tree from the chain
synced 0 leaves, root 0x14f44d672eb357739e42463497f9fdac46623af863eea4d947ca00a497dcdeb3
  before: 0 leaves, 0 transfers, shielded 0, unshielded 0, bal:A 0, bal:B 0, root:note 14f44d672eb35773…
== 1. shield: A mints 100 publicly, burns it into a deposit, and proves the claim
note 0 commitment 0x0977ba66a42ec9f85fa0abd2031df56242a0ac5fa8653d456364ad15c1b988ef
  status {'Ready': {'reported_in': {'heade
  ok   bal:A after the mint (+100) = 6400000000000000
  status {'Ready': {'reported_in': {'heade
  ok   bal:A after the shield (-100) = 0000000000000000
  ok   dep:C, arrived by deferred transfer = 6400000000000000
payload 8642 bytes -> /Users/austinkloske/Projects/abutlabs/Aiden/submodules/jam-privacy-service/out/tx1.hex
  status {'Ready': {'reported_in': {'heade
  ok   root:note, change 1 of 3 (the claim) = 02be96587523d86dcfb06c26554aabfd8c8853deb863fdf79f1d38b41ec4894d
  ok   dep:C consumed = (none)
== 2. send: A proves a payment of 40 to B, keeping 60
payload 8642 bytes -> /Users/austinkloske/Projects/abutlabs/Aiden/submodules/jam-privacy-service/out/tx2.hex
  status {'Ready': {'reported_in': {'heade
  ok   root:note, change 2 of 3 (the send) = 0d68250c7c46d7bfc267aaf6dfed61deae5d6fd288fd41131c8e6790f14239c2
== 3. unshield: B proves a spend of its 40 to public account B on service 1
payload 8642 bytes -> /Users/austinkloske/Projects/abutlabs/Aiden/submodules/jam-privacy-service/out/tx3.hex
unshield 40 -> account 2 on service 1; deposit 0
  status {'Ready': {'reported_in': {'heade
  ok   root:note, change 3 of 3 (the unshield) = 1afa4c9544456145e2cae8c9ca651457794cf4bce534840a8bc528ad25dc6c32
== the note root: three changes, each the builder's, and the chain's leaves give it back
  ok   three distinct changes: 14f44d672eb3… → 02be96587523… → 0d68250c7c46… → 1afa4c954445…
  ok   each proof starts from the root the one before left
  ok   leaf 0 = tx1's note hash 1 = 0babcf5d4bd8426696920c9802445caaca165b341309ca42faef83df6112b859
  ok   leaf 1 = tx1's note hash 2 = 22ebe06c99539f3ba35aa1da9b7c4b052b6ad642db79ce9a1e5988de2daacba4
  ok   leaf 2 = tx2's note hash 1 = 0df152cf0873a21ff28f40b6f187c25eebfe457a4689b7cdc2197b5ea932634b
  ok   leaf 3 = tx2's note hash 2 = 1fd67a20e4334946c28a281d17f03044549fbbf54da5f8a92025f36ffdc20ca6
  ok   leaf 4 = tx3's note hash 1 = 0a313652f84ad4be328b35e7db05bf04707fc817e4b480fab37c407f97027bf9
  ok   leaf 5 = tx3's note hash 2 = 00575ca0cf9999cfd648502b1e5b237a34d915b4ede48cde5eff0d01a9b8d8d4
  ok   leaves = 6 = 0600000000000000
  ok   root rebuilt from the chain's 6 leaves = root:note = 1afa4c9544456145…
== nullifiers: exactly these six, and the nullifier root chains through them
  ok   nf: tx1 input 1 = 01
  ok   nf: tx1 input 2 = 01
  ok   nf: tx2 input 1 = 01
  ok   nf: tx2 input 2 = 01
  ok   nf: tx3 input 1 = 01
  ok   nf: tx3 input 2 = 01
  ok   root:null chains from before → tx1 → tx2 → tx3: nothing else was spent between them
  ok   root:null = tx3's = 284010731cf4103ddf2d88f927ea1ec2c398dca5f00565857fe4ab222faf4550
== balances and counters
  ok   bal:A: +100 minted, -100 shielded = 0000000000000000
  ok   bal:B: +40, by deferred transfer = 2800000000000000
  ok   transfers +3 = 0300000000000000
  ok   shielded +100 = 6400000000000000
  ok   unshielded +40 = 2800000000000000
== replay the send: a valid proof whose nullifiers are spent
  status {'Ready': {'reported_in': {'heade
  ok   root:note unchanged = 1afa4c9544456145e2cae8c9ca651457794cf4bce534840a8bc528ad25dc6c32
  ok   leaves unchanged = 0600000000000000
  ok   transfers unchanged = 0300000000000000
  ok   bal:B unchanged = 2800000000000000
== what each side holds
note 2: A 0 (tree, leaf 1)
note 4: A 60 (tree, leaf 3)
note 5: B 0 (tree, leaf 4)
note 6: B 0 (tree, leaf 5)
PASS
nets/demo.sh check  11.81s user 3.14s system 7% cpu 3:13.26 total
```
`nets/demo.sh shield`, `send`, `unshield` (after the formatter fix; A already held the
faucet's 100 publicly):
```
$ nets/demo.sh shield
A shields 100
  deposit note: commitment 2cf705a5c2d3…
  shield: package b0c0d6065a8a… on core 0
  shield: on chain after 12 s
  claim: proof of 8642 bytes in 0.5 s
  claim: package dfef225d0195… on core 0
  claim: on chain after 22 s
  done in 35.5 s: A public 0, private 100
$ nets/demo.sh send
A sends 40 to B privately
  send: proof of 8642 bytes in 2.4 s
  send: package 5695ef12b29b… on core 0
  send: on chain after 26 s
  done in 28.8 s: A public 0, private 60
$ nets/demo.sh unshield
B unshields 40
  unshield: proof of 8642 bytes in 2.5 s
  unshield: package 7640a12319b7… on core 0
  unshield: on chain after 22 s
  done in 25.0 s: B public 80, private 0
```
`node client/e2e/e2e.mjs` → two browser contexts, A and B, exit 0:
```
start: A {"public":0,"private":60}, B {"public":80,"private":0}
  screenshot docs/images/01-a-start.png
== A: faucet, mint 100 publicly
  done: mint 100 in 14.9 s
  ok   A after mint: public 100 (want 100), private 60 (want 60)
  screenshot docs/images/02-a-minted.png
== A: shield 100 (a deposit by deferred transfer, then a proof that claims it)
  done: shield 100 in 74.6 s
  ok   A after shield: public 0 (want 0), private 160 (want 160)
  screenshot docs/images/03-a-shielded.png
== A: send 40 to B privately
  done: send 40 to B in 23 s
  ok   A after send: public 0 (want 0), private 120 (want 120)
  screenshot docs/images/04-a-sent.png
== B, in the other browser: the 40 has arrived
  ok   B after receiving: public 80 (want 80), private 40 (want 40)
  screenshot docs/images/05-b-received.png
== B: unshield 40 to B's public account
  done: unshield 40 in 29.1 s
  ok   B after unshield: public 120 (want 120), private 0 (want 0)
  screenshot docs/images/06-b-unshielded.png
PASS: 5 assertions, 6 screenshots
```
Accumulate for a complete send, in-process rig (`demo_deploy.exe privacy-service.jam <tx2>`):
`refine gas used : 181910244 steps: 59229019`, `accumulate gas used : 103167`.

### The demo-ready conditions (docs/RALPH.md), against this run
1. `up` from clean starts six lasair validators, deploys both services (public-balance 1,
   privacy 2), and starts the indexer, the builder container and the wallet; `down` left
   0 containers and no wallet. The package-building "builder" is the indexer's submit path,
   one proof a package, no folding (README, reduced).
2. `check` passed with the assertions listed above: three root changes equal to the
   prover's and to the root rebuilt from the chain's leaves; the six nullifiers and the
   chained nullifier root; A −100 from the shield, B +40; the replayed send dropped.
3. Real UltraHonk proofs from Noir via bb 3.0.3, verified in refine on lasair's PVM: about
   182 million gas (181,910,244 on the rig here; 182,164,641 in the guarantor's log), in
   this ledger and the README.
4. The wallet, `jam/` in the demo-wallet fork, did shield, send and unshield for A and B in
   two browser contexts against this net; screenshots `docs/images/01…06` are from this run.
5. README has *Demo* with `up`, `check`, `shield`, `send`, `unshield`, `down`, and
   *What is real and what is reduced*.
6. This entry is the verification run.
Issues: #8 and #10 closed; #4 (the pj6 soak) and #14 (lasair vs PolkaJam) stay open, as
does #17 (jamboree). Still reduced: #15, #18, #19, lasair#93.
Next: milestone 4, an hour's soak on pj6; then the reductions, starting with #19 (proving in
the browser) and #18 (the Aztec kernel).
Blockers: none. The net, indexer and wallet are left running for Aodh to try.

## 2026-10-23 10 — phase 2 begins
Did: Aodh asked to close every open issue and make the demo ready for the JAM implementers'
channel, testnet improvements included. `docs/RALPH.md` gained *Phase 2* with a closing
condition per issue (#4, #11, #12, #13, #14, #15, #17, #18, #19), lasair#93, polish and CI.
Next: #15 first, since #19 and #18 move the circuit and the toolchain: fork zkVerify's
verifier, find what bb 5.0.0 changed in the UltraHonk proof (7,232 bytes zk against 7,488),
accept it, measure it on the PVM, and move the toolchain to Noir 1.0.0-rc.5 + bb 5.0.0.
Blockers: none.

## 2026-10-23 11 — #15: bb 5.0.0 proofs, verified on the PVM; the toolchain moves
Did: generated bb 3.0.3's and bb 5.0.0's Solidity verifiers for one circuit and diffed them
(normalised: 126 hunks): the pairing-point object is 8 limbs instead of 16 (the 256 bytes),
the ZK masking commitment moves, the padding constant is 25 instead of 28. Found zkVerify's
open PR #9 "Update to Barretenberg v5.0.0" (branch lg/bb-v5, 14 commits, head a4b2a4af5d);
forked the verifier to abutlabs/ultrahonk_verifier with that head on `bb-v5`. Verified bb 5
proofs on the host, then parametrised the spike over both verifiers (`bb3`, `bb5` features)
and metered both on lasair's rig. Moved everything to Noir 1.0.0-rc.5 + bb 5.0.0: Poseidon
library v0.4.0 (same hashes: `H(7,11,13)` = `0x1c7467e2…` under both compilers), `u1` bits →
`bool` in the circuit, the client calls pinned binaries (`~/.nargo/versions`, `~/.bb/versions`,
overridable) and bb 5's flow (`write_vk -t evm` once, `prove -t evm -k`), the service
depends on the fork. Re-ran phase 1 from clean.
Evidence: host: `bb5 zk: Ok(()) in 5.67ms`, `bb5 plain: Ok(())`, tampered `ok=false` both.
`spikes/ultrahonk-gas/measure.sh` → bb5 zk 286,136,021 (01), plain 272,022,666 (01), flipped
509,110 (00), wrong input 9,748,240 (00); bb3 zk 312,260,095, plain 302,381,955.
`services/privacy/check.sh` → `PASS: 6 assertions`, refine `gas used : 167281732 steps:
53793187`. Proving a transfer: 0.5 s (was 2.5 s); payload 8,386 bytes. From clean on lasair6:
`nets/demo.sh up` 1 min 47 s → `services: public-balance 1, privacy 2`; `check` → `34 ok,
0 FAIL`, `PASS`, 2 min 34 s; `node client/e2e/e2e.mjs` → `PASS: 5 assertions, 6 screenshots`.
Issues: #15 closed, #11 closed, #13 (run 4).
Next: #13: explain the spike's 286M against the service's 167M for the same verifier (the
spike's circuit is smaller), by metering both with log_n and public-input count varied;
then the README's final gas table, and close #13.
Blockers: none. Net, indexer and wallet left running on bb 5.

## 2026-10-23 12 — #13: the verifier's gas explained, and the profile fixed
Did: the spike (286M) and the service (167M) differed by build profile, not circuit: the
spike built with `opt-level = "z"`, the service's profile was ignored by Cargo (`warning:
profiles for the non root package will be ignored`) and it built at opt-level 3. Metered one
bb 5 zk proof under five profiles. Moved the profile to the workspace root (opt-level 3,
LTO, one codegen unit), removed the ignored member profiles, set the spike the same.
Evidence: vks: spike `log_n 12 num_pub 9`, transfer `log_n 14 num_pub 20`. Profiles, zk gas:
z+LTO 286,136,021 · s+LTO 238,812,374 · 2+LTO 161,038,053 · 3+LTO 159,389,725 · 3 no LTO
162,842,117. After the change: no profile warning; `services/privacy/check.sh` → refine
`166132009`, `PASS: 6 assertions`; `measure.sh` → bb5 zk 161,500,875, plain 152,871,367;
bb3 zk 177,314,601, plain 171,198,181; rejections 0.33 to 6.1M. From clean: `up` 1 min 49 s,
`check` → `34 ok, 0 FAIL`, `PASS`, 2 min 57 s.
Issues: #13 closed.
Next: #12: `scripts/bootstrap.sh`, which checks and installs the pinned toolchain on macOS
and Linux (Rust + the riscv target, Noir rc.5 and bb 5.0.0 into the versioned paths, Node,
Python, Docker present, the sibling checkouts, jam080-build and lasair's demo_deploy built),
then `docs/TOOLS.md` generated from the same pins; run it here, and in a clean Linux
container for everything but Docker.
Blockers: none. Net, indexer and wallet left running.

## 2026-10-23 13 — #12: one script installs the pinned toolchain
Did: `scripts/bootstrap.sh`: the pins live at its top and nowhere else; it checks and
installs Rust (stable + nightly-2025-05-10 with rust-src for jam080-build), Noir rc.5 and
bb 5.0.0 into the versioned paths, checks Node 20+, Python 3.11+, openssl, Docker and
Chrome, clones `../jamswap` and `../demo-wallet` (on `jam`), builds jam080-build, compiles
both circuits, builds both services, installs puppeteer-core and pulls the lasair image.
`--check` reports only; `--rig` adds lasair's in-process rig (OCaml 5.4.0 via opam, the FFI
crates, demo_deploy; abutlabs/lasair is private, and only check.sh and the spike use it);
`--tools-md` generates `docs/TOOLS.md`. The README's prerequisites point at it. Ran it here
and in a clean `node:22-bookworm` container (Linux aarch64) that clones this repo from GitHub.
Evidence: this Mac → 20 rows `ok`, `Ready`, 3.9 s; `--rig` → `lasair rig  OCaml 5.4.0  built ok`.
Clean Linux container (`scripts/bootstrap.sh`, from nothing, 2 min 55 s):
```
TOOL                     WANTED                     FOUND                              STATUS
curl                     any                        /usr/bin/curl                      ok
git                      any                        /usr/bin/git                       ok
rust                     stable                     1.99.1                             ok
rust nightly             nightly-2025-05-10         present                            ok
nargo                    1.0.0-rc.5                 1.0.0-rc.5                         ok
bb                       5.0.0                      5.0.0                              ok
node                     >= 20                      v22.23.4                           ok
python3                  >= 3.11                    3.11.2                             ok
openssl                  any                        present                            ok
docker                   daemon running             missing                            missing
chrome                   any (the e2e run only)     missing; set CHROME=               optional
../jamswap               checkout                   44213a7                            ok
../demo-wallet           jam branch                 8f9c22b                            ok
jam080-build             jamswap tools/jam080       built                              ok
circuit transfer         compiles                   ok                                 ok
circuit hash_cli         compiles                   ok                                 ok
service public-balance   builds                     13702 bytes                        ok
service privacy          builds                     1015546 bytes                      ok
puppeteer-core           25.16.0                    25.16.0                            ok

1 missing. Install the rest by hand (see docs/TOOLS.md), then run this again.
```
Docker cannot run inside the container; Chrome is optional (the e2e run only). Noted: the
privacy blob is 1,015,546 bytes on Linux and 1,015,513 on the Mac, so the build is not yet
byte-reproducible across machines (jamswap found Cargo hashes absolute paths); a polish item.
Issues: #12 closed.
Next: #4: the demo on the PolkaJam-only net. `nets/pj6.sh` as the counterpart of
`nets/lasair6.sh` over jamswap's `nets/compose/pj6.yml` (or its `./dex up NET=pj6`), and a
`NET=` switch in `nets/demo.sh` so up, check and the stages run on lasair6 or pj6; run
`check` on pj6. The hour's soak comes after.
Blockers: none.

## 2026-10-23 14 — #4, part one: the demo on six PolkaJam validators
Did: `nets/lasair6.sh` became `nets/net.sh`, chosen by `NET=lasair6` (default) or `NET=pj6`
(jamswap's `nets/compose/pj6.yml`: six PolkaJam validators and an ordinary PolkaJam node
serving JIP-2). It uses jamswap's project names and reads the RPC node's network and the
`/shared` volume from `docker compose config` instead of hard-coding them. `nets/demo.sh`
passes `NET` through; the indexer reports the net and the wallet page shows it. Ran the
demo on pj6, then on lasair6 again under the renamed project.
Evidence: `NET=pj6 nets/demo.sh up` → `service 1 ready (created #9500015)`, `service 2
ready (created #9500020)`, `indexer up on pj6`, 1 min 14 s. `NET=pj6 nets/demo.sh check` →
`34 ok, 0 FAIL, 0 resends`, `PASS`, 1 min 46 s. `node client/e2e/e2e.mjs` on pj6 → `PASS: 5
assertions`, 1 min 18 s (mint 10.1 s, shield 27.1 s, send 18.2 s, unshield 18.3 s). Then
`nets/demo.sh up` (lasair6) 1 min 25 s, `check` → `34 ok, 0 FAIL, 0 resends`, 3 min 05 s.
Issues: #4 (part one), #14 (first numbers: the same check, pj6 1:46, lasair6 3:05).
Next: #4, part two: the hour's soak on pj6: `nets/soak.sh` sends one privacy package a slot
(a pre-proven chain of sends between A and B, so proving never paces it), records every
root change, package status and finalized slot, and ends with a verdict: zero dropped roots,
finality advancing. Then close #4.
Blockers: none. lasair6 left running.

## 2026-10-23 15 — #17, part one: `jamboree rpc`, a Rust JIP-2 client in our jamboree fork
(While the soak ran; RALPH's long-run exception: code-only, nothing the soak used.)
Did: forked jamixir/jamboree (abutlabs/jamboree, branch `lasair`, submodule in Aiden) and
added `jamboree rpc`: head, authorizer, params, services, service, read, submit [--core]
[--wait], status, deploy, a port of jamswap's offchain/jip2.py, workpackage.py, chain.py,
chainspec.py and deploy.py (src/rpc: codec, wp, spec, client, mod); dispatched before
jamboree's own CLI parsing so nothing of jamboree changes; a README section.
Evidence: `cargo test rpc` → 6 passed, including the work-package bytes against the hex
jamswap's workpackage.py produces for the same package. On the live pj6 net (read-only):
`jamboree rpc --url ws://127.0.0.1:42150 head` → `best 9500281 … finalized 9500280 …`;
`services` → `0 1 2`; `read 2 transfers` → `6000…` (96, the indexer's count); `read 2 root:note`
→ the indexer's root; `--chain-spec pj6 authorizer` → `0:f8d86b97…  cores 0,1`, the same as
chainspec.py's. Not yet run live: submit and deploy (after the soak).
Next for #17: submit/deploy live, then jamboree recipes for native lasair and PolkaJam nets.

## 2026-10-23 16 — #18, part one: sealed notes in the protocol, scanning, the directory; a void soak
Did: the circuit gained `logs_hash` (13 public inputs); refine checks blake2b(the two sealed
notes) against it and accumulate stores each as `enc:<leaf>`; the client seals both outputs
(client/notecrypt.py: X25519 + HKDF-SHA256 + AES-256-GCM via `cryptography`, RFC 7748
vectors, a WebCrypto cross-check; #20 opened and closed when the homebrew first draft was
replaced before use); public-balance op 3 publishes a 64-byte directory entry (encryption
key || spend key); the indexer serves `/notes`, `/key`, sealed notes in `/state`; the client
`scan`s; `check` runs A and B as separate clients sharing only the chain. Poseidon2 moved
in-process (client/poseidon2.py, Noir rc.5's constants; the solver's 5 vectors, hash_cli's
outputs and the empty root match): 0.55 s a transfer, a 1,200-leaf tree in 0.34 s.
The void soak: my circuit change reached the first hour-long soak through the shared
`circuits/transfer` directory at 636 s (`prove_error … Expected argument logs_hash`) and it
starved; I stopped it. Two defects it showed, both fixed: every client process now proves
in its own circuit snapshot (two run in parallel cleanly; the snapshot is removed at exit),
and the soak's verdict fails on prove errors, starved slots or a pace under 90%.
Evidence: `services/privacy/check.sh` → `PASS: 8 assertions` (a swapped sealed note is
refused), refine 166,237,540 gas; offline scan: B finds 40, a fresh A finds 60, C nothing, a
note swapped to another leaf is refused; rig: `key:<B>` holds the 64-byte entry.
Not yet run live: the new `check` (after the soak).

## 2026-10-23 17 — #19, part one: the browser can prove, seal and scan by itself
Did: in the demo-wallet fork's `jam/`: noir_js 1.0.0-rc.5 and bb.js 5.0.0; lib/poseidon2.js
(the same constants and vectors), lib/blake2b.js (RFC 7693; Python's hashlib at 12 lengths),
lib/notecrypt.js (WebCrypto, byte-compatible with the Python), lib/core.js (identity from a
random secret: spend key, encryption key, account; notes, tree, transfer inputs, payload, scan).
Evidence: `node test-bbjs.mjs` → witness 72 ms, `proof 8000 bytes, 13 public inputs, in 728
ms`, bb.js's vk byte-identical to the service's; the service on the rig → `VALID`, 166,168,063
gas. `node test-core.mjs` (a JS wallet with its own secret claims 100 and pays the Python
client's B 40 through B's directory entry) → proof in 355 ms; rig `VALID`; the JS wallet
scans and finds its 60 at leaf 1; B's Python client scans and finds `B 40 at leaf 0`.
Next for #19: the page itself on lib/core.js (bundled), proving in the browser, and the
two-browser run with no wallet backend.

## 2026-10-23 18 — #18, part two: an indexed nullifier tree in the circuit, depth 40, a genesis hole closed
Did: nullifier non-membership is proven in the circuit with Aztec's indexed Merkle tree: each
nullifier's bracketing low leaf (membership), the rewrite of its pointer, and the new leaf in
an empty slot at the tree's size (a public input the service checks and advances); the
service records nullifiers in order (`nfl:<i>`) and the indexer serves them, so any client
rebuilds the tree (Python `IndexedTree`, JS `IndexedTree`). Designing it showed a soundness
hole, now closed: with empty storage the service had accepted any old root, so the first
proof on a new chain could start from a tree of its own (a fake note) — the service now
starts from the true empty trees (constants). Measured depth before choosing it
(`spikes/depth/measure.py`) and moved both trees to Aztec's 40. Also: a default argument had
frozen the client's tree depth (fixed); `check.sh` now fails if the blob is too small to
hold the verifier (LTO compiles the verifier out when the embedded vk is stale: seen twice).
Evidence: depth table — 16: 1,464 opcodes, 0.72 s, 167,620,188 gas; 24: 1,960, 0.81 s,
167,578,114; 32: 2,456, 1.12 s, 168,826,687; 40: 2,952, 1.19 s, 168,929,285 (all VALID).
At 40: `services/privacy/check.sh` → `PASS: 10 assertions` (incl. `accumulate refuses: a new
chain starts from the empty trees`), refine 169,112,761 gas; `scripts/offline-proof.sh` →
`bb verify: the proof holds …`, `… a changed public input is refused`; four chained transfers
→ the indexed tree's linked list visits every leaf in order, the client's root = the proof's;
`node test-core.mjs` → the JS wallet's proof `VALID` on the rig, B's Python client finds 40.
Next: after the soak, everything live on pj6 and lasair6 (the new circuit and service, the
two-client check, the self-custodial browser run), then the Aztec kernel spike.

## 2026-10-23 19 — #4: an hour on pj6, one privacy package a slot, nothing lost (milestone 4)
Did: `client/soak.py` (a prover thread chaining real zero-value transfers, a builder that
sends package k+1 once k is Reported, resends on silence, an observer, a verdict that fails on
any drop, prove error, starved slot, pace under 90% or finality stall over 60 s) for 3,600 s on
pj6 at commit 76aac7c. The first attempt is void (ledger 16: my circuit change reached it).
A first 3-minute trial without the Reported gate lost a package to reordering (14 landed, 16
dropped after it), which is why the gate exists: on one tree, packages must land in order.
Evidence: `client/soak.py --seconds 3600 --out out/soak-hour` → `soak exit 0`:
```
| submitted | 600 |  | landed | 600 |  | dropped | 0 |  | resends | 0 |
| latency_s | {'p50': 13.52, 'p90': 15.01, 'p99': 15.87, 'max': 16.15} |
| prove_s_p50 | 0.661 |  | packages_per_minute | 9.97 |  | pace_vs_one_a_slot | 1.0 |
| prove_errors | 0 |  | starved_slots | 0 |
| best | [9500427, 9501028] |  | finalized | [9500426, 9501027] |  | finality_longest_stall_s | 10.1 |
| verdict | PASS |
```
(report: docs/soak-pj6.md; events: out/soak-hour/events.jsonl.)
Issues: #4 closed; lasair#93 given PolkaJam's count; #14 (the pj6 half of the comparison).
Next: the live runs held back by the soak, on a clean pj6 and lasair6: the new circuit and
service (indexed nullifier tree, depth 40, sealed notes), `check` with two clients, the
stage commands, and the self-custodial browser run; then #14's lasair runs (reader against
builder, testing lasair#93's cause), the jamboree native net, and the Aztec kernel spike.

## 2026-10-23 20 — the new circuit and service live on pj6; the browser's first proof no longer waits on a CDN (#19)
Did: pj6 down and up from clean with the depth-40, indexed-nullifier-tree service (1,028,608
bytes) and the self-custodial wallet, then `check`, the stage commands and the recorded
two-browser run. Four things the runs found, fixed:
- `down` returned while the observability stack's collectors still held the net's Docker
  network; the next `up` created nodes on a network netjoin then removed ("network pj6_net not
  found"). `nets/net.sh down` now detaches whatever is left and removes the network itself.
- The stage client counted notes `check` had spent (same name-derived keys): scan marked spent
  only from a `spent` list the indexer never sends. It now uses the indexer's nullifiers and
  re-marks known notes whose nullifiers are on chain. ("A holds 260" became the true 160.)
- `send` scanned for the receiver before the indexer had polled the block ("B holds 0"):
  each proved stage now waits for the indexer to reach the root it just checked.
- The wallet's first proof took ~80 s of a 108.6 s shield: bb.js fetched 20 MB of CRS from
  Aztec's CDN, single-threaded because the page was not cross-origin isolated. Now
  `scripts/fetch-crs.sh` caches exactly the ranges bb.js 5.0.0 asks for (sha256-pinned, the
  first 4 MB equal to bb's native cache), the wallet server serves them at /crs/ with byte
  ranges and COOP/COEP, and demo-wallet's `build.mjs` points bb.js at the page's origin (Aztec's
  S3 kept as fallback). Barretenberg.new 13.9 s → 2.3 s; vk 853 → 262 ms; no request leaves
  the machine.
Also: each client lists only its own notes with value; `client/genesis_check.py` (in CI)
asserts the client's empty trees equal the service's genesis constants.
Evidence:
`NET=pj6 nets/demo.sh check` → `PASS` in 2 min 11 s (three root changes each rebuilt from the
chain's leaves, six nullifiers, nullifier root chaining, B finds exactly its 40, replay
changes nothing). Stages: `send` → "B's client scans the chain: found note 6: B 40 at leaf 12 /
A holds 80, B holds 40 privately"; `unshield A 20` → "A holds 60 privately, 20 publicly".
`RECORD=1 node client/e2e/e2e.mjs` → before: shield 108.6 s, run 3:32; after:
```
done: mint 100 in 12.1 s / shield 100 in 25.8 s / send 40 in 13.3 s / unshield 40 in 13.9 s
PASS: 5 assertions, 6 screenshots; every proof made in its own browser
recording docs/images/demo.mp4 and demo.gif (both browsers side by side)   (1:37 total)
```
`scripts/bootstrap.sh --check` → every row ok incl. "prover CRS … cached, hashes match".
Commits: demo-wallet 9db6880; this repo (below).
Issues: #19 closed (the browser holds its key, proves in the page, receives by scanning
sealed notes; two contexts share only the chain; wallet.py is a static server).
Next: the same on a clean lasair6 (check, stages, e2e); then #14's lasair runs testing
lasair#93's cause, the jamboree native net (#17), the Aztec kernel spike (#18).

## 2026-10-23 21 — lasair6 with the new service, and lasair#93's real cause
Did: lasair6 (ghcr.io/abutlabs/lasair:2.1.3) down and up from clean with the depth-40
service; `check`, the stage commands and the browser run. All pass; lasair loses packages.
Evidence:
`NET=lasair6 nets/demo.sh check` → `PASS` in 6 min 54 s (pj6: 2 min 11 s), three resends;
`docker logs lasair6-lm0-1 | grep -c EXPIRED` → 3, the other five nodes 0. Stages → pass
(A 160 → 120, B +40, B unshields 40), two resends, JIP-2 status `Failed: the anchor is not in
…` for each: the issue's second symptom (Ready for a package never reported) does not recur
on 2.1.3. `node client/e2e/e2e.mjs` → `PASS` (mint 12.4 s, shield 30.5 s, send 27.5 s,
unshield 16.1 s; 2 min 10 s).
The cause, traced on package 0xdf76fa9bc23b846a across the six logs: lm0 (validator 0)
accepted it, shared it over CE-134 with validators 3, 4 and 5, got three matching signatures
within 40 ms, and did that again every slot for 38 s without ever distributing over CE-135;
then EXPIRED (anchor_too_old). lm0 was on core 0 at the head but not at the head's child,
the slot the pipeline builds the guarantee for: intake (`Wp_intake.authorize`) accepted on
assignment at the head or the next slot; `Coguarantee.credential` counts only a rotation in
which our own key signs (lasair#81), and the shares went to the next rotation's guarantors.
About one package in R = 4 arrives in the last slot of a rotation. My earlier reading in
lasair#93 (a guarantee waits for the holder's own authoring slot) was wrong: since lasair#60
the pipeline distributes every tick.
Fix (lasair branch `intake-at-guarantee-slot`, 0422962): intake checks head + 1 by default;
the co-signer keeps head .. head + 3. New test `lasair#93: refused when our assignment ends
at the head` fails on the old window ("accepted a package that should be refused") and passes
on the new. `dune test --force` → 72 suites "Test Successful", 0 `[FAIL]`;
`scripts/quic-tests.sh` → 4 ok.
Next: the conformance gate's result; images from main and from the branch; a lasair6 soak
on each (one package a slot), counting expiries; then lasair#93 and #14.

## 2026-10-23 22 — lasair#93 fixed for its main cause (lasair #95), measured; the rest is lasair#94
Did: in ../lasair, intake now takes a builder's package only if we are on its core at the
head's child (`Wp_intake.authorize ?earliest ?lookahead`, defaults 1 and 1; the co-signer
keeps 0..3); two tests (refused at a rotation turn, red on the old window; a previous-rotation
credential inside an epoch). Gate, images from main (`lasair:93-before`) and from the branch
(`lasair:93-after`, Dockerfile.mesh), then the same 20-minute soak on a clean lasair6 with
each (`out/ab93.sh`). PR #95 merged (5203a8f); lasair#93 closed; lasair#94 opened.
Evidence: `dune test --force` → 72 "Test Successful", 0 `[FAIL]`; `scripts/quic-tests.sh` →
4 ok; `scripts/conformance.sh` → 48 ok, 2 skip, exit 0.
```
| image        | submitted | landed | expired (resends) | pace  | latency p50 / p90 / p99 |
| main         | 62        | 62     | 14 (lm0 12, lm1 2) | 0.31  | 21.9 / 44.9 / 62.5 s    |
| main + fix   | 77        | 77     | 5  (lm0 4, lm3 1)  | 0.385 | 21.2 / 30.1 / 55.5 s    |
| pj6 (ledger 19) | 600    | 600    | 0                  | 1.0   | 13.5 / 15.0 / 15.9 s    |
```
After the fix the nodes refused 58 packages `not_assigned` and the reader passed each to the
next guarantor (lm0 17 taken, lm1 10, lm2 5, lm3 3, at mid-run). Both soaks "FAIL" the
verdict on pace (under 90% of a slot) and resends, not on drops: every package landed.
(reports: docs/soak-lasair6-main.md, docs/soak-lasair6-fix.md.)
The five left: accepted on the core, two co-signatures within a second, but core 0 still
held the previous report (the pipeline waits for a free core at the head); the rotation
turned meanwhile and the re-shares went to guarantors it cannot sign with. Inside an epoch
the previous-rotation credential covers that (the new test); across an epoch lasair does not
time a guarantee in the previous rotation, though GP allows it (P(eta'_3, lambda')). I could
not pin the five turns to slots (this chain's slots do not track wall time), so lasair#94
says "consistent with", and pairs it with the pace gap the same wait causes.
The price: the demo still pulls ghcr.io/abutlabs/lasair:2.1.3 (publishing a lasair image is
Aodh's call) and keeps its resend after 36 s, which is safe (payloads apply once).
Next: #14 and #17 together: the native nets jamboree launches (jb-pj6, jb-lasair6, jb-mixed)
with `nets/jb.sh`, the demo's check on each, and refine wall time per client from the mixed
net's logs; then the comparison table.

## 2026-10-23 23 — #17: the demo on three native nets jamboree launches; a mixed-net fork found and explained
Did: `nets/jb.sh` grew from one untested net to three, each launched by our jamboree fork
(`--move-ports`) on PolkaJam's dev spec, deployed and driven with `jamboree rpc`, read by the
indexer's own JIP-2 client: nothing of jamswap. `jb-pj6` (six PolkaJam + an ordinary node for
JIP-2), `jb-lasair6` (six lasair at main with #95 + lasair-reader), `jb-mixed` (PolkaJam 0-2,
lasair 3-5). `jb.sh headcheck` runs jamboree's own check (validators headless, JIP-3 to
jamboree, `--head jip3-final`). What the runs found, fixed:
- jamboree ran each `--cmd` from the net's directory: `$0` must be absolute.
- A native net answers before finality leaves genesis (dev genesis at slot 0, first block at
  today's wall slot), and a package then is refused ("invalid epoch 791829, reference epoch
  is 0"): `up` waits for finality.
- lasair alone must run chain-paced (`--interval 6`, no `--wall`), as jamswap's lasair6 does;
  wall-paced it "lags wall slot 9502050" and back-fills forever. Its pipeline and builder port
  need `--guarantor`.
- **The mixed net forked**: lasair rejected PolkaJam's block 9502227 (`bad_parent_state_root`).
  `jamnp_probe --dump` (lasair #96, new) read both clients' whole state at the last common
  block over CE-129: 33 keys each, one byte apart, in key 0x0d (activity statistics): a
  service's accumulate gas, PolkaJam 17,475, lasair 17,477. lasair main charges the v0.8.0
  vectors' cmov rule (063fe3a, GP PR 559); PolkaJam nightly-2026-09-22 predates it. With
  PolkaJam **v0.8.0** (2026-10-16) the mixed net agrees: native nets now pin v0.8.0
  (bootstrap fetches it). Docker nets keep jamswap's pair (lasair 2.1.3 + the 09-22 nightly),
  which agree with each other.
- PolkaJam's default finality is a dummy (best block finalized alone): in a mixed net lasair's
  three voters never reach 5 of 6 (0 finalized in 3 minutes). PolkaJam now runs
  `--finality-mode grandpa` on the native nets, as on Docker pj6: lasair then finalizes with it.
- `check` failed when rerun on a used chain (B also holds earlier runs' notes): it now counts
  only the notes this run made. `demo.sh wallet` waited 20 s for a route the static server
  no longer has.
Evidence (all on this Mac, Apple M-series):
```
jb-pj6     up + deploy (jamboree rpc)   check PASS 1:40 (37)  e2e PASS: mint 12.1 s, shield 25.8, send 11.1, unshield 15.6
jb-lasair6 up 26 s to finality          check PASS 3:06 (37)  e2e PASS: mint 16.5 s, shield 50.3, send 19.3, unshield 27.8
jb-mixed   (v0.8.0)                     check PASS 1:41 (37)  e2e PASS: mint 10.1 s, shield 25.7, send 13.1, unshield 17.8
jb-mixed   (v0.8.0, GRANDPA)            check PASS 1:36 (37); lasair node 4: 35 blocks finalized, 0 rejected
jb-pj6     check rerun on a used chain  PASS 1:28 (37)
NET=jb-mixed|jb-pj6|jb-lasair6 nets/jb.sh headcheck 3 → "same head on 6 nodes after 180s", exit 0, each
```
The jamboree fork's README now carries a lasair recipe and a mixed one; both run as written
(`--headless 2m --head jip3-final` appended) → "same head on 6 nodes after 120s".
Refine of one privacy proof (169M gas), both interpreted on macOS: PolkaJam v0.8.0 median
0.172 s over 18 (its JIP-3 `Refined` events, `JB_TELEMETRY` + jam-telemetry-dumper); lasair
(native dev build) 0.63 to 0.97 s over 10 (its logs).
Not yet: the demo's check on jb-pj6 since PolkaJam moved to GRANDPA (the head check passed);
the final verification will run it.
Issues: #17 (all but the upstream offer, next); #14 gets these numbers.
Next: the upstream issue on jamixir/jamboree; then #14's table and the README; then #18.

## 2026-10-23 24 — #17 closed: every net through jamboree, the offer upstream
Did: the Docker nets now use the same tooling as the native ones. jamboree built for Linux
(`scripts/jamboree-linux.sh`, rust:1.88-bookworm, rebuilt when ../jamboree moves) runs
`jamboree rpc` in a container on the net, whose JIP-2 endpoint is not published to the host;
the indexer has one backend (indexer/jip2.py reads; submissions through `jamboree rpc
submit`, or lasair's builder with SUBMIT_VIA_BUILDER); `nets/ctl.sh` holds the head, read and
submit both net scripts use. `nets/jamctl.py` and the indexer's jamswap backend are deleted:
`git grep offchain` finds only the plan's history. Two things the switch exposed: `up` must
wait for finality to leave genesis (jamswap's deploy.py had waited; `jamboree rpc` does not),
and JIP-2 calls a landed package `Reported` where jamswap said `Ready`.
The jamboree fork's README has lasair and mixed recipes (run as written: "same head on 6 nodes
after 120s"), its tests pass (`cargo test --locked`: 31), and the offer is
jamixir/jamboree#49 (the one public post the loop makes).
Evidence:
`NET=pj6 nets/demo.sh up` → ready in 1:05; `check` → PASS 1:51 (37); stages → "A holds 120,
B holds 40 privately … B holds 0 privately, 80 publicly"; `node client/e2e/e2e.mjs` → PASS
(mint 12.1 s, shield 23.9, send 11.2, unshield 11.8; 1:22); indexer: "submit via jamboree rpc".
`NET=lasair6 nets/demo.sh up` → 1:43; `check` → PASS 3:38 (37), no resend.
`scripts/bootstrap.sh --check` → "jamboree (Linux) for the Docker nets built ok".
Issues: #17 closed; jamixir/jamboree#49 opened.
Next: #14 (the comparison table), the README, #18 (the Aztec kernel spike), then the final
verification on every net.
