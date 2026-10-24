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

## 2026-10-23 25 — #18 closed: Aztec's own kernels measured; the port is roadmap #21
Did: the rest of #18 (the demo-visible reductions went in ledger 16 to 18: sealed notes, the
indexed nullifier tree, depth 40). `spikes/chonk/measure.sh`, from scratch: Aztec CI's pinned
benchmark stacks (aztec-packages v6.0.0, chonk-inputs afe8389cb7f21295) proved with bb 6.0.0;
one proof verified and timed on one thread against ours; the kernels compiled from Aztec's
v6.0.0 source (sparse checkout, Aztec's workspace generator) with nargo 1.0.0-rc.3.
Evidence: `spikes/chonk/measure.sh` → exit 0 (49 s):
```
11 flows proved: 6 to 12 circuits each, 2.16 to 8.19 s on 10 threads, 80,096 or 128,192 bytes
the ecdsar1+transfer_0_recursions+sponsored_fpc proof verifies (a flipped byte fails at Merge reduction)
UltraHonk (ours, 8,768-byte proof): 2.3 ms CPU; CHONK (Aztec's, 80,096 bytes): 164 ms; ratio 70x
estimated CHONK verifier in refine: 11.8 billion gas (a full package holds 5, a tiny one 1)
private_kernel_init_3      29,146 opcodes, 78,019 gates   (Aztec's stack: 29,131 / 78,007)
private_kernel_reset_tail  89,717 opcodes, 312,773 gates  (the stack's is a sized variant)
hiding_kernel_to_rollup    1,420 opcodes, 40,502 gates    (identical bytecode)
```
Two earlier runs gave ratios of 72 and 75 to 79: about 12 billion gas (11.8 to 13.4).
The verdict (spikes/chonk/RESULTS.md): a kernel-based port needs a builder that wraps each
CHONK proof (Aztec's prover, relocated), Aztec's trees in accumulate, and the PXE; beyond a
loop, so it is roadmap issue #21, the only issue left open under RALPH's #18 rule.
Issues: #18 closed; #21 opened (`roadmap`).
Next: #14's table into the README (and close #14), the README rewrite, DEMO.md and
ANNOUNCE.md final, then the final verification on every net.

## 2026-10-23 26 — the final verification, every net from clean; #14's table
Did: `scripts/verify-all.sh` (new): on each net from a clean state, `down`, `up`, `check`, the
three stage commands, the two-browser run, `down`; then jamboree's head check on each native
net. lasair6 and pj6 are the nets RALPH names; the three native nets ran too. The first full
run passed both Docker nets and failed the native ones at once: my port guard in `nets/jb.sh`
(`lsof` exits 1 when the ports are free, which `pipefail` + `set -e` turned into a silent exit),
and the default PolkaJam path picked the Linux build now cached beside the macOS one. Both
fixed; the native nets rerun; the head checks run separately (I had edited the script under the
running bash); pj6 rerun to record the README's images (the first run's recording was
overwritten by the script's `git checkout docs/images`, now replaced by `IMAGES=` for runs that
should not touch them).
Evidence (`out/verify-all.log`, `out/verify-jb.log`, `out/verify-pj6-rec.log`; logs in
out/verify/):
```
lasair6     down      ok      0s  
lasair6     up        ok     96s  indexer up on lasair6: slot 18 leaves 0 transfers 0
lasair6     check     ok    189s  PASS
lasair6     shield    ok    199s    A holds 160 privately, 0 publicly
lasair6     send      ok     24s    A holds 120, B holds 40 privately
lasair6     unshield  ok     23s    B holds 0 privately, 80 publicly
lasair6     e2e       ok    178s  PASS: 5 assertions, 6 screenshots; every proof made in its own browser
lasair6     down      ok     11s  
pj6         down      ok      1s  
pj6         up        ok     75s  indexer up on pj6: slot 9503442 leaves 0 transfers 0
pj6         check     ok    115s  PASS
pj6         shield    ok     39s    A holds 160 privately, 0 publicly
pj6         send      ok     18s    A holds 120, B holds 40 privately
pj6         unshield  ok     18s    B holds 0 privately, 80 publicly
pj6         e2e       ok     97s  PASS: 5 assertions, 6 screenshots; every proof made in its own browser
pj6         down      ok     13s  
jb-pj6      down      ok      2s  
jb-pj6      up        ok     73s  indexer up on jb-pj6: slot 9503519 leaves 0 transfers 0
jb-pj6      check     ok     97s  PASS
jb-pj6      shield    ok     35s    A holds 160 privately, 0 publicly
jb-pj6      send      ok     12s    A holds 120, B holds 40 privately
jb-pj6      unshield  ok     13s    B holds 0 privately, 80 publicly
jb-pj6      e2e       ok     89s  PASS: 5 assertions, 6 screenshots; every proof made in its own browser
jb-pj6      down      ok      2s  
jb-lasair6  down      ok      2s  
jb-lasair6  up        ok    106s  indexer up on jb-lasair6: slot 19 leaves 0 transfers 0
jb-lasair6  check     ok    171s  PASS
jb-lasair6  shield    ok    119s    A holds 160 privately, 0 publicly
jb-lasair6  send      ok     26s    A holds 120, B holds 40 privately
jb-lasair6  unshield  ok     23s    B holds 0 privately, 80 publicly
jb-lasair6  e2e       ok    116s  PASS: 5 assertions, 6 screenshots; every proof made in its own browser
jb-lasair6  down      ok      3s  
jb-mixed    down      ok      2s  
jb-mixed    up        ok    127s  indexer up on jb-mixed: slot 9503676 leaves 0 transfers 0
jb-mixed    check     ok     96s  PASS
jb-mixed    shield    ok     41s    A holds 160 privately, 0 publicly
jb-mixed    send      ok     13s    A holds 120, B holds 40 privately
jb-mixed    unshield  ok     18s    B holds 0 privately, 80 publicly
jb-mixed    e2e       ok     90s  PASS: 5 assertions, 6 screenshots; every proof made in its own browser
jb-mixed    down      ok      2s  
jb-pj6      headcheck ok   same head on 6 nodes after 180s: 0x012514d2154d2f203c2d6091a
jb-lasair6  headcheck ok   same head on 6 nodes after 180s: 0xde8ee8f823a4ea11a475feda0
jb-mixed    headcheck ok   same head on 6 nodes after 180s: 0x3c9a65f2b54060bf77f79419c
pj6 (recorded) e2e: mint 12.1 s,shield 25.8 s,send 11.2 s,unshield 16 s; recording docs/images/demo.mp4, demo.gif
```
Resends (a package that went silent and was sent again): lasair6 2 over check and the stages,
jb-lasair6 1, the PolkaJam and mixed nets none. Two-browser moves: lasair6 shield 81.3 s, send
17.5 s; pj6 25.8 s, 11.2 s; jb-lasair6 32.2 s, 13.3 s; jb-pj6 25.7 s, 13.2 s; jb-mixed 25.8 s,
13.2 s.
#14's table is in the README (*lasair and PolkaJam, side by side*): refine of one proof,
interpreted, Linux aarch64 containers, lasair 0.94 s against PolkaJam 0.207 s; macOS native
0.65 s against 0.172 s; PolkaJam's recompiler on CI's x86-64 runner 8 to 24 ms in 15 of 30
refines and 0.41 to 0.56 s in 12 (not a node's first refine; cause not known); lasair audits a
privacy report in 1.0 to 1.9 s (PolkaJam reports no audit telemetry); one tree at one package a
slot, 0.385 against 1.0; check and the wallet's moves as above.
Issues: #14 closed.

## 2026-10-23 27 — Phase 2 done, condition by condition
1. **Every issue in the table is closed with a closing comment that links its evidence:** #19
   (ledger 20), #18 (16 to 18, 25), #15, #11, #13, #12 (phase 2's first entries), #14 (26), #4
   (19), #17 (23, 24). Open: only #21, the `roadmap` issue #18's rule allows (a port on Aztec's
   own kernels: CHONK verification about 12 billion gas by ratio, so a builder that wraps each
   proof).
2. **lasair#93 is closed with a fix** (lasair #95, merged as 5203a8f: intake takes a builder's
   package only on the head's child's assignment; expiries 14 in 62 → 5 in 77 over 20 minutes),
   the rest filed as lasair#94; the README says so in *lasair and PolkaJam, side by side*.
3. **The final verification** (ledger 26): lasair6 and pj6 from clean, `down`, `up`, `check`,
   the stage commands and the e2e run all pass; so do the three native nets, and jamboree's
   head check on each.
4. **docs/DEMO.md, docs/ANNOUNCE.md** (final, no draft notes), **the recording**
   (docs/images/demo.mp4 and demo.gif, pj6, 2026-10-23), **the CI workflow** (builds both
   services and the circuits, checks the cryptography against vectors, proves and verifies a
   transfer offline, and deploys the service on a PolkaJam testnet and times its refine): green
   on abcd6f9, and on this entry's commit (checked before the promise).
Other repos, all pushed: demo-wallet `jam` 9db6880; jamboree `lasair` b64d5d4 (offered upstream
as jamixir/jamboree#49); lasair main 592fb8f (#95, #96).

## 2026-10-23 28 — #4 reopened: the hour fails on the shipped code; the client's quadratic tree, fixed; the hour passes
**Correction to ledger 27.** Condition 1 counted #4 closed on ledger 19's hour, which loaded its
client at 76aac7c (about 02:02). The indexed nullifier tree (5cd5539, 02:22) and depth 40
(1213a57, 02:25) were committed during that hour, so it never ran them, and nothing re-ran it
after. The README, DEMO.md and ANNOUNCE.md presented its 600 of 600 as the shipped code's. #4
and #12 are reopened (#12: `scripts/bootstrap.sh` was verified from nothing at 3799fce, before
it gained PolkaJam, jamboree, the CRS and the rig; only `--check` since); #14 has a correction.
Did, in order:
- `NET=pj6 nets/demo.sh up`: the first start's GRANDPA voters broadcast before their peers
  connected and never finalized; `up` waited 300 s, then deployed anyway into "invalid epoch"
  refusals. `up` now stops with a message (937d933). The second start finalized normally.
- A 120 s trial (`--out out/soak-trial`): PASS, 20 of 20, prove p50 1.5 s.
- The hour at 1bbd96e (`out/soak-hour-2`): **FAIL**, 137 submitted, 137 landed, 0 dropped,
  finality's longest stall 10.6 s, but prove p50 22.3 s, 453 starved slots, pace 0.228. The
  client rebuilt the indexed tree per proof, each insert rehashing the whole tree twice:
  50, 100, 200, 400 nullifiers took 2.5, 7.6, 26.5, 95.7 s (Poseidon2 in Python, 0.28 ms a hash).
  The wallet page's `IndexedTree` was the same code (100 nullifiers: 4.3 s in Node).
- Fixed (97f5c8e; demo-wallet 8f6ded8): the list is linked first (low leaf by bisection) and
  hashed once, then each insert rehashes only its two leaves' paths (`Tree.set`); a long-lived
  prover keeps both trees between proofs and hashes only what the chain added, rebuilding when
  the chain's list diverges. Leaves, roots, witnesses and duplicate refusals equal the old
  code's on random and small values, in Python and JS; 2,000 nullifiers build in 1.1 s
  (Python) and 0.66 s (JS), the same root. `client/tree_check.py` (in CI) checks every
  witness opens the roots the circuit checks, the kept trees, the JS root, and the build time.
- The hour again, pj6 from clean, client at 97f5c8e (`out/soak-hour-3`): `soak exit 0`
```
| submitted | 600 |  | landed | 600 |  | dropped | 0 |  | resends | 0 |
| latency_s | {'p50': 13.43, 'p90': 14.98, 'p99': 15.78, 'max': 16.31} |
| prove_s_p50 | 0.744 |  | packages_per_minute | 9.96 |  | pace_vs_one_a_slot | 1.0 |
| prove_errors | 0 |  | starved_slots | 0 |
| best | [9505409, 9506011] |  | finalized | [9505408, 9506010] |  | finality_longest_stall_s | 10.1 |
| verdict | PASS |
```
- On the net it left (1,200 notes, 1,200 nullifiers): `NET=pj6 nets/demo.sh check` PASS twice
  (37 ok each, 2 min 07 s), `shield`, `send`, `unshield` each ok, `client/e2e/e2e.mjs` "PASS: 5
  assertions, 6 screenshots; every proof made in its own browser".
README, DEMO.md and docs/soak-pj6.md now carry this run (13.4 s median, 0.74 s a proof).
Issues: #4 closed on this entry; #14 corrected; #12 open until `scripts/bootstrap.sh` runs from
nothing again. Still to rerun on the fix: lasair6's 20 minutes (its prover was never starved,
0 starved slots, so its pace is the chain's, but it ran the quadratic client).

## 2026-10-23 29 — #12: scripts/bootstrap.sh from nothing again, at 99f8b62
Did: `scripts/bootstrap-clean.sh` (new: a fresh `node:22-bookworm` container, apt's python3,
git, curl, openssl and a C toolchain, then this repo cloned from GitHub and
`scripts/bootstrap.sh`), on this Mac's Docker (Linux aarch64). Since ledger 13 the script
gained the Python dependency, jamboree, the native PolkaJam release, the wallet's provers and
the CRS fetch; this run covers all of them.
Evidence: 298 s from clone to report, 24 rows:
```
cryptography             >= 41                      50.0.2                             ok
docker                   daemon running             missing                            missing
chrome                   any (the e2e run only)     missing; set CHROME=               optional
../demo-wallet           jam branch                 8f6ded8                            ok
../jamboree              lasair branch              b64d5d4                            ok
service privacy          builds                     1028605 bytes                      ok
wallet provers           noir_js + bb.js            built                              ok
prover CRS               bb.js 5.0.0 ranges         cached, hashes match               ok
jamboree                 lasair branch, built       jamboree 0.7.0                     ok
polkajam (native)        v0.8.0                     polkajam 0.1.29                    ok
1 missing. Install the rest by hand (see docs/TOOLS.md), then run this again.
```
(and curl, git, rust stable 1.99.1 and nightly-2025-05-10, nargo rc.5, bb 5.0.0, node 22, python
3.11, openssl, jamswap 44213a7, jam080-build, both circuits, public-balance 14,057 bytes,
puppeteer-core: ok). The one missing row is Docker, which cannot run in the container, so the
two Docker-only steps (pulling the lasair image, building jamboree for Linux) are covered by
this Mac's `--check` (ledger 24), not from nothing. The privacy blob is 1,028,605 bytes here
and 1,028,608 on the Mac: still not byte-reproducible across machines (ledger 13).
Issues: #12 closed.

## 2026-10-23 30 — lasair6's 20 minutes again, on the fixed client: the pace was the chain's; the fix helps less than one pair said
Did: ledger 22's A/B (`out/ab93.sh`: a clean lasair6, `client/soak.py --seconds 1200`, image
`lasair:93-before` then `lasair:93-after`) with the client at 99f8b62. The first try of the
"after" arm failed in `up`: `jamboree rpc deploy` created the privacy service and sent its
preimage, then "Error: No such file or directory (os error 2)" writing its state file, though
`out/state/` existed and held the first service's. jamboree's fork now names the path in that
error (lasair branch); the arm's second try deployed normally, so the cause is not known
(possibly the Docker Desktop bind mount).
Evidence (reports: docs/soak-lasair6-main-2.md, docs/soak-lasair6-fix-2.md):
```
| image       | run       | submitted | landed | expired (EXPIRED lines) | resends | pace  | p50 / p90 / p99 s  | prove p50 |
| main        | ledger 22 | 62        | 62     | 14                      | 14      | 0.31  | 21.9 / 44.9 / 62.5 | 4.56 s    |
| main        | rerun     | 65        | 65     | 9 (lm0 9)               | 10      | 0.325 | 21.8 / 41.9 / 74.5 | 0.99 s    |
| main + fix  | ledger 22 | 77        | 77     | 5                       | 5       | 0.385 | 21.2 / 30.1 / 55.5 | 6.14 s    |
| main + fix  | rerun     | 75        | 75     | 7 (lm0 4, lm1 1, lm3 2) | 8       | 0.375 | 21.2 / 35.0 / 48.7 | 1.04 s    |
```
With proofs five times faster, pace and latency did not move: lasair's pace is the chain's
(the guarantor waiting for a free core, lasair#94), as #14 said. The fix's effect is smaller
than ledger 22's one pair suggested: expiries 14 and 9 without it, 5 and 7 with it, pace 0.31
and 0.325 against 0.385 and 0.375. README, DEMO.md, ANNOUNCE.md and the tutor's JAM beside
Aztec page now give both pairs, or "about 0.38 of a slot".
Issues: #14 commented.

## 2026-10-23 31 — Aztec on JAM: refine checks Aztec's epoch-proof shape on PolkaJam for 153–166M gas; the verifier ported to bb 6.0.0; Phase 3 begins
Did:
- **Direction (#28).** JAM as Aztec's settlement layer: refine checks Aztec's one epoch
  proof, as Ethereum's Rollup contract does, and Aztec's kernels, PXE, node and prover run
  unchanged. This replaces #21's per-transaction CHONK check (about 12B gas). Aztec's node
  and contracts were mapped (#25, #26); the PXE needs no change; the sequencer publisher
  and `RollupContract` have no interface; blobs are EIP-4844 KZG (#24).
- **The verifier (#22).** Ported to bb 6.0.0 on abutlabs/ultrahonk_verifier `bb-v6`
  (2ae44db): the challenge derivation and the new ROM-LogUp relation, taken from bb's own
  Solidity diff. Fixtures were regenerated (`scripts/generate_test_fixtures.sh`), 44 of 46
  tests pass, and the invalid-inner pair waits on a bb without its self-check.
- **The epoch proof on PolkaJam (#23).** Aztec's flags, 113 public inputs, synthetic circuits
  2^12 to 2^22, on a six-validator polkajam-testnet v0.8.0 (Aodh: Aztec work on PolkaJam,
  not lasair).
Evidence: `spikes/epoch-proof/measure.sh` → refine gas 153,250,436 (2^12), 155,653,349,
158,581,383, 161,085,556, 163,676,071, 166,433,017 (2^22); PolkaJam refine 0.157–0.181 s
(interpreter, Darwin arm64); accumulate's verdicts 01; a public input altered → 00 at
9,456,761 gas. The 2^24 attempt swapped for 50 minutes in `nargo execute` and was stopped.
- **Issues.** #22–#28 (`aztec-on-jam`); Aodh's privacy brief as #29–#35
  (`privacy-research`, `jip`).
- **The loop.** `docs/RALPH.md` Phase 3: what closes each issue, and the rules (PolkaJam;
  simulated personas; nothing submitted).
Next: the loop reviews the open issues and starts by dependency order.

## 2026-10-23 32 — #24: EIP-4844's point evaluation costs refine 199,469,377 gas on PolkaJam
Why this issue: it is first in dependency order. Its numbers feed #31 (whether JAM's lack of
native cryptography is a real limit), #26 (the settlement service) and #29 (lessons), and it
depends on nothing open.
Did: `spikes/kzg-gas`. c-kzg-4844's `verify_kzg_proof` plus the 0x0a precompile's
versioned-hash check, in no_std Rust on zkcrypto's bls12_381 0.8, with Ethereum's [s]G2.
Checked on the host against 122 c-kzg vectors and geth's precompile vector (pinned commits
a751e2d and 2006f08), then measured in refine on a PolkaJam testnet. `spikes/lib/polkajam.sh`
is now shared by both spikes and waits for the nodes to be connected, not just finalized: one
run's deploy was refused at "0 peers". The epoch-proof spike was rerun through it with
identical gas.
Evidence: `spikes/kzg-gas/measure.sh` → "c-kzg verify_kzg_proof vectors: 122 agree, 0
differ"; geth Ok(true), y altered Ok(false); on PolkaJam every case 199,469,377 gas (19.9%
of a tiny package, 4.0% of a full one), 0.19 s, verdicts 01 for the four valid and 00 for the
two invalid. `spikes/epoch-proof/measure.sh` → 153,250,436 … 166,433,017, as in ledger 31.
Beside Ethereum: the precompile's fixed 50,000 gas is about 0.1% of a block; here 4% of a
full package (#31).
Also: Aodh found Aztec asking the community for Noir verifiers on other chains, including
Polkadot (x.com/aztecnetwork/status/1641394426005315587; not opened from here). Noted on #35
and #22 as a lead.
Next: #24's second half, `verify_blob_kzg_proof` per blob (binding the data to its
commitment, which Ethereum's consensus does for free), and where blob data lives on JAM.
Blockers: none.

## 2026-10-23 33 — #24: binding a blob to its commitment costs 262,430,040 gas; PolkaJam caps packages near 256 KiB (#36)
Why this issue: it continues #24, still first in dependency order. The per-blob check is the
half of Aztec's blob handling that Ethereum's consensus does for free.
Did: c-kzg-4844's `verify_blob_kzg_proof` added to `spikes/kzg-gas/kzg`: the Fiat-Shamir
challenge, barycentric evaluation over the bit-reversed 4,096th roots of unity (7^((r-1)/4096))
with one batched inversion, then the pairing check. The service gained a kind byte, so one
blob covers both checks; `spikes/lib/polkajam.sh` sends raw payloads as @file.
Evidence: `spikes/kzg-gas/measure.sh` → host "verify_blob_kzg_proof vectors: 29 agree, 0
differ", "verify_kzg_proof vectors: 122 agree"; on PolkaJam, blob checks 262,430,040 gas
(26.2% of a tiny package), 0.26 s, verdicts 01, 01 and 00 for the incorrect proof; point
evaluation 199,473,075 (+3,698 for the dispatch byte).
Found: packages of 262,339 bytes or more are refused by every core ("Failed to submit
work-package to even a single proxy/guarantor"), while 131,171 bytes passes. That is under
the Gray Paper's W_B of 13,791,360 (definitions.tex:285). Filed as #36.
Arithmetic (not measured): an Aztec epoch's 192 blobs, checked one by one, would cost about
50B gas (ten full packages), so data binding is the expensive part of settlement.
Next: #24's last part (where blob data lives on JAM, and batch verification), or #36's
threshold. #24 first.
Blockers: none.

## 2026-10-23 34 — #24 closed: Aztec's blob data lives in JAM's exported segments (28 days, past Ethereum's 18.2)
Why this issue: it is first in dependency order, and its last row item (where blob data
lives) is reading and deciding, with no new run needed.
Did: `docs/aztec/BLOBS.md`. Four options, each with Gray Paper 0.8.0 sources:
- the package's own Audits DA, kept until audited (`assurance.tex:5`): too short alone;
- exported segments, kept 28 days (`assurance.tex:7`), 32 a blob, 96 blobs of exports a
  package (`definitions.tex:287,292`): **chosen**;
- service storage at 1 per byte (`definitions.tex:259`), 25 MB a full epoch: rejected for
  unbounded state;
- off chain only: rejected, weaker than Ethereum gives Aztec.
KZG binding is kept so Aztec's circuits stay unchanged. A hash-committed variant goes to #30
as an alternative.
Evidence: Ethereum's window `MIN_EPOCHS_FOR_BLOB_SIDECARS_REQUESTS = 4,096` epochs
(consensus-specs 3dad3d5, `deneb/p2p-interface.md:69`) = 4,096 × 32 × 12 s = 18.2 days, under
JAM's 28. The point evaluation (199,473,075 gas) and the blob check (262,430,040) are from
ledger 33, and the KZG code is unchanged since.
Issues: #24 closed. #37 opened for the export path and batch verification on PolkaJam.
Next: by dependency order, #36 (the package cap, which bounds #37 and #26) or #23's 2^24
with a heavy-opcode circuit. #36 first: it is small and it changes #26's design.

## 2026-10-23 35 — #36 closed: PolkaJam's cap is JAM-NP's 200 KiB work-package message; blobs go as extrinsics
Why this issue: it bounded both #37 (how many blobs a package carries) and #26 (the
settlement service's package layout), and a probe settles it in one testnet session.
Did: a binary search of payload sizes on one polkajam-testnet v0.8.0. Each payload was
[tag][kind 2][filler], which the kzg-gas service refuses at once.
Evidence:
- The probe → "largest accepted payload: 204533 bytes; smallest refused: 204534 bytes".
  Refused packages fail on every core: "Failed to submit work-package to even a single
  proxy/guarantor".
- 204,533 + 267 bytes of package encoding = 204,800 = 200 KiB.
- JAM-NP (zdave-parity/jam-np e0b7d29, `simple.md` lines 362 and 383): "The initial message
  `Core Index ++ Work-Package` should be limited to `2 + (200 * 1024)` bytes", in CE-133 and
  CE-146, so a guarantor can check authorization before downloading more.
- Extrinsic data travels in a separate message, bounded by the Gray Paper's W_B =
  13,791,360 (`definitions.tex:285`).
Consequence: payloads hold at most about 200 KiB a package (one blob); a checkpoint's blobs go
as extrinsics, about 100 a package. `docs/aztec/BLOBS.md` and `spikes/kzg-gas/RESULTS.md`
say so.
Issues: #36 closed; #37 gains the extrinsic route (`jamboree rpc submit` with extrinsics, and
the service fetching them).
Next: #37 (blobs as extrinsics, exported as segments, measured on PolkaJam). It is now
unblocked and is the last measurement #26 needs.

## 2026-10-23 36 — #37: blobs as extrinsics, exported as segments, on PolkaJam; gas, not size, sets blobs per package
Why this issue: #36 showed blobs must travel as extrinsics, and #37 is the last
measurement #26 (the settlement service) needs.
Did:
- `jamboree rpc submit` gained `--extrinsic` (repeatable, referenced by (blake2b-256,
  E4(length)), GP C.29, sent as JIP-2's third argument) and `--exports`, with a test pinning
  the encoding (jamboree lasair, pushed).
- The kzg-gas service gained kind 3: blobs read from extrinsics, KZG-checked, optionally
  exported as 32 segments each.
- `spikes/lib/polkajam.sh`'s `pj_send` passes extra jamboree arguments.
Evidence: `spikes/kzg-gas/measure.sh` → 1 blob as an extrinsic 261,895,733 gas (verdict 01);
the same exported as 32 segments 262,013,216 (+117,483); 3 blobs exported as 96 segments
786,027,114 (78.6% of a tiny package, 0.73 s, 01); 4 blobs out of gas at 999,999,331 (no
verdict). Packages of 401 to 798 bytes carried 131,072 to 524,288 bytes of extrinsics. All
earlier cases unchanged: point evaluation 199,473,081, blob as payload 262,364,258, invalid
00. Host: 122 and 29 c-kzg vectors agree.
Consequence: exporting is cheap (about 3,700 gas a segment), and gas limits blobs per package
to 3 tiny or 19 full. An Aztec checkpoint's 6 blobs fit one full package; an epoch's 192 take
about 10 (`docs/aztec/BLOBS.md`).
Next: #37's import path (a later package importing a blob's segments) and batch verification.

## 2026-10-23 37 — #37: batch-checked, an Aztec checkpoint's 6 blobs fit one tiny package (967,993,405 gas)
Why this issue: it is still unblocked and first in order. Batching is the cost lever for an
epoch's 192 blobs, and it is self-contained code with Ethereum's own vectors.
Did: c-kzg-4844's `verify_blob_kzg_proof_batch` added to `spikes/kzg-gas/kzg`: the
"RCKZGBATCH___V1_" challenge over (C, z, y, pi) for every blob, then one pairing over a random
linear combination. Each blob's opening is shared with the single check (`blob_opening`). The
service gained kind 4, a batch over extrinsic blobs.
Evidence: `spikes/kzg-gas/measure.sh` →
- host: "verify_blob_kzg_proof_batch vectors: 24 agree, 0 differ", with 29 and 122 for the
  others;
- PolkaJam: 2 blobs 404,223,059; 3 blobs 545,169,223 (01), and 00 with one wrong proof; 6
  blobs 967,993,405 (96.8% of a tiny package, 0.93 s, 01). The line from 2 and 3 (122,330,731
  + 140,946,164 a blob) predicted 968,007,715 for 6.
Consequence: a checkpoint's 6 blobs fit a tiny package; an epoch's 192 cost about 27.2B gas
(5.4 full packages) against 50.4B one by one.
Next: #37's import path (a later package importing a blob's 32 segments), the last item
before #37 closes.

## 2026-10-23 38 — #37 closed: a blob exported by one package is imported and re-checked by another, on PolkaJam
Why this issue: #37's import path was its last item. It is the round trip that keeps Aztec's
data in JAM's DA for 28 days and readable by later packages.
Did:
- jamboree gained `submit --import root:<hash>:<i> | pkg:<hash>:<i>` (GP C.30, tested).
- jamboree gained `report <hash>`: JIP-2 workReport's package spec, decoded per GP 0.8.0. My
  first decode left out E2(erasure shards) (`serialization.tex:208-215`) and read 18,549
  exports from the bytes; fixed, it reads exports 32, shards 6.
- The kzg-gas service gained kind 5: import n segments, rebuild the blob, check it.
Evidence: `spikes/kzg-gas/measure.sh` → imported by segments root: 263,028,551 gas, 3
guarantors, verdict 01; with a wrong proof 00. Imported by work-package hash: refined once
(primary only), "Not reported in time", node1 "Not enough signatures from secondary
guarantors" (filed as #38). All earlier cases unchanged (batch of 6: 967,993,490).
Issues: #37 closed. #38 opened, and added to RALPH's table.
Next: by dependency order, #23 (the epoch proof at 2^24, with a heavy-opcode circuit), then
#26 (the settlement service), now that its inputs are measured.

## 2026-10-23 39 — #25 closed: Aztec's Ethereum boundary mapped, and four seams drafted as an unsent proposal
Why this issue: an independent step while #23's 2^23 proof runs (RALPH allows one that does
not touch what the run uses). It reads code, needs little memory, and #26 needs it.
Did: `docs/aztec/BOUNDARY.md`, from aztec-node 1ac42dc and aztec-packages v6 nightly:
- what runs unchanged: the PXE, the circuits, proving, world state, P2P;
- every Ethereum touchpoint with its interface status;
- the 14 publisher methods the sequencer calls and the 14 Rollup reads the archiver, epoch
  cache and prover node make;
- four seams drafted for Aztec, not sent: `SettlementPublisher`, `SettlementReader`,
  `SettlementSource`, and settlement-neutral stdlib types.
The earlier agent map was checked against the code. The class and interface locations held,
but four line counts did not and were replaced by direct counts: `ethereum/src` 11,277 (not
about 13k), archiver `l1/` and `modules/` 4,944 (not 3.7k), prover-node publisher 473 (not
521), blob client 2,002 (not 838).
Evidence: `grep -n "^export class RollupContract"` → rollup.ts:288 (1,502 lines);
`SequencerPublisher` sequencer-publisher.ts:167, taken by type at sequencer.ts:768; TXE runs
on `TXEArchiver` and `MockEpochCache`; the PXE reads `getNodeInfo` at
pxe/src/entrypoints/server/utils.ts:40; the method lists come from grep over
`sequencer-client/src/{sequencer,client}` and the Rollup callers.
Issues: #25 closed.
Next: #23's 2^23 result (running), then #26.

## 2026-10-23 40 — #27 closed: v5.2.1 → v6.0.0 at the boundary, measured
Why this issue: an independent step while #23's 2^23 proof runs. It is network reads and
diffs, and #26 needs to know how fast the Rollup's logic moves.
Did: `docs/aztec/RELEASES.md`. The boundary files fetched at v5.2.1 and v6.0.0 from
aztec-packages, and the TypeScript from v5.2.1's yarn-project against aztec-node 1ac42dc
(aztec-node has no tags, and 1ac42dc is its last commit before v6.0.0), then diffed.
Evidence:
- `HonkVerifier.sol`: v5.2.1 29 subrelations, 111+8 public inputs; v6.0.0 31, 113+8;
  `LOG_N = 24` in both.
- The two new inputs are previous and end inbox rolling hash (`ROOT_ROLLUP_PUBLIC_INPUTS_LENGTH`).
- Changed lines: ProposeLib 182, EpochProofLib 360, RollupCore 182, Inbox 273, Outbox 0,
  rollup.ts 80, sequencer-publisher.ts 379 (same methods), and `RollupContract` gains
  `getAttesterCountAtTime`.
- v6's main change is a streaming Inbox (a rolling-hash bucket ring).
Conclusion: the prover changes every major release (a small verifier port each time, done for
v6 in #22), the Rollup's logic changes most, and the node's TypeScript boundary barely moves.
Issues: #27 closed.
Next: #23's 2^23 result, then #26.

## 2026-10-23 41 — #23: 2^23 measured on PolkaJam (176,083,620 gas); the circuit's gate mix moves the cost
Why this issue: #23 is the open measurement nearest the head of the order. 2^23 was the
largest size this 16 GB machine could prove (#23's first 2^24 attempt swapped).
Did: from 2^23, `spikes/epoch-proof/measure.sh` builds the circuit from keccakf1600 calls
(about 15,100 gates each, measured with `bb gates`) instead of an unrolled loop, so the
program stays small. A 40-minute watchdog was set; it was not needed.
Evidence: `LOGS=23 ./measure.sh` →
- "2^23 proof 10240 B, 113 public inputs, prove 54.08 s, peak 5582 MB; bb and our verifier
  accept";
- PolkaJam refine 176,083,620 gas (3.5% of a full package), 0.180 s, verdict 01; a public
  input altered 9,625,506, 00.
`out/c23/src/main.nr`: N = 416 calls.
Found: 2^23 costs 9.65M more than 2^22, whose squaring circuits gained about 1.3M a
doubling. The proof grew by exactly one round, so the jump is the gate mix: zero selector
commitments are cheaper for the verifier. Our transfer circuit (many gate types) costs 169M
at 2^14 with 20 public inputs, against 155.6M for the bare one with 113. So "about 169M at
2^24" understated a real root rollup. RESULTS.md says so.
Also: `docs/aztec/SERVICE.md` (#26's design) drafted; #39 filed (secp256k1 ECDSA in refine,
for proposer signatures and attestations).
Next: #23 with a circuit that uses every relation (lookups, ROM, Poseidon2, elliptic) across
sizes, to bound the real root rollup's cost; then 2^24 if memory allows. Then #26's skeleton.

## 2026-10-23 42 — #39 closed: ecrecover costs refine 13.1M gas a signature; #23: every relation in use, about 180M at 2^24
Why these: #23's every-relation series was running when this iteration began, and #39 was the
last per-package cost #26 needs. Each used the testnet in turn.
Did:
- `spikes/ecdsa-gas`: Ethereum's ecrecover on k256 (no_std). geth's valid vector has a high
  s, which the precompile accepts and k256 does not, so a high s is normalized and the
  recovery id flipped.
- `spikes/epoch-proof` gained `CIRCUIT=rich`: keccakf1600, Poseidon2, embedded-curve
  addition, ROM/RAM, secp256k1 ECDSA and range checks in one circuit (about 17,500 gates a
  block on 43,400 fixed, from `bb gates`).
Evidence:
- `spikes/ecdsa-gas/measure.sh` → host "geth ecRecover vectors: 5 agree, 0 differ",
  "generated signatures: 40 recover their signer". PolkaJam: 1 signature 13,119,449 gas (01);
  33 signatures 432,756,815 (8.7% of a full package, 0.52 s, 01); the 17th swapped
  222,947,408 (00); InvalidHighV 6,379 (00).
- `CIRCUIT=rich LOGS="16 18 20 22 23" spikes/epoch-proof/measure.sh` → 170,364,998;
  172,760,359; 175,583,937; 178,078,520; 179,184,883 (2^23, 01); altered 9,631,420 (00).
Consequence: an epoch-proof package is about 180M (proof) + 199M (blob point evaluation) +
433M (attestations), about 812M; a checkpoint is about 981M. Both fit a tiny package. 2^24
(with every relation) is now proving, with a 60-minute watchdog.
Issues: #39 closed and added to RALPH's table; #23 commented.
Next: #23's 2^24 result; then #26's skeleton; then #29 (LESSONS.md is drafted, pending
verified Ethereum comparisons).

## 2026-10-23 43 — #29: LESSONS.md drafted, its Ethereum comparisons checked; #40 filed (Aztec's benchmark uses a mock verifier)
Why this issue: #23's 2^24 is proving (memory-bound), and #29 is reading and writing. Its
inputs from #23, #24, #37 and #39 are measured.
Did: `docs/privacy/LESSONS.md`, Aodh's first question answered with each claim linked to its
evidence. Checking the draft's Ethereum figures changed three of them:
- The block gas limit is 60M (Fusaka, EIP-7935), not the draft's 36M.
- "25 to 2,000 times" was unfounded. Like with like (share of the budget each check runs in),
  JAM pays about 48× (point evaluation) and 52× (ecrecover). Per chain, JAM has about 14×
  the room: 341 packages of 5e9 a 6 s slot against one 60M block a 12 s slot.
- The UltraHonk verifier's Ethereum cost had no source. Aztec's `gas_benchmark.md` (908,757
  for `submitEpochRootProof`) runs a mock verifier (`test/benchmark/happy.t.sol:732`), so
  it is to be measured in #40.
Evidence: `l1-contracts/test/benchmark/happy.t.sol:732` "the mock verifier still needs
production-sized calldata"; sources for the gas limit listed in LESSONS.md.
Issues: #40 filed and added to RALPH's table; #29 stays open until #40's row lands in
LESSONS.md.
Next: #23's 2^24 result; #40 in Foundry after it (memory); then #26's skeleton.

## 2026-10-23 44 — #31 closed: a JIP is warranted, for one limit (no cryptographic host functions)
Why this issue: #23's 2^24 is still proving (the machine is in swap), and #31 is reading and
deciding on evidence that is already measured. #32, #33 and #34 wait on it.
Did: `docs/privacy/LIMITS.md`. Six candidate limits, each marked spec-level or not, with its
evidence and a verdict. The Gray Paper 0.8.0 host-call list (`pvm_invocations.tex`) has no
hash, curve or pairing call. zk.money's history was checked against sources: Aztec announced
the Aztec Connect sunset on 13 March 2023 (deposits off 31 March 2023, blocks stopped 31 March
2024) as a focus on its general-purpose network. Neither source names an L1 wall, so the file
does not claim one, and narrows the parallel to what the evidence supports.
Decision: a JIP for cryptographic host functions (BN254 and BLS12-381 pairings and MSM, KZG
point evaluation, secp256k1 recovery, and a hash if #41 shows accumulate needs it), priced
from benchmarks. No privacy-specific protocol features.
Evidence: 199,473,075 (KZG, #24), 13,119,449 (ecrecover, #39), 170–180M (UltraHonk, #23),
about 48–52× Ethereum's share of the budget at a 60M block (LESSONS.md); the host-call list
from `grep -oE "\\texttt\{[a-z_]+\} = [0-9]+" pvm_invocations.tex`.
Issues: #31 closed. #41 filed (Poseidon2 in the PVM, for one tree shared by many packages) and
added to RALPH's table.
Next: #23's 2^24 when it finishes; then #40, #41 and #26 on PolkaJam, one testnet at a time;
#32 (the JIP draft) can start meanwhile: its first step is researching the JIP format.

## 2026-10-23 45 — #32 step 1: how JIPs are written and proposed
Why this issue: #31 decided a JIP is warranted, and #33 and #34 wait on the draft. Its first
step is the format, which needs no testnet.
Did: `docs/jip/FORMAT.md`, from polkadot-fellows/JIPs at 5202db1: the README, JIP-1 to
JIP-5, and the pull requests (#12 README, #13 and #14 JIP-6 and JIP-7 in flight, #17
amendments).
Evidence: `gh api repos/polkadot-fellows/JIPs/contents` → JIP-1..5 and README; JIP-1's
host-call layout (Index 100, Name `log`, Gas usage, Input registers φ7…, Output registers
φ'7 = WHAT, Side-effects).
Next: `docs/jip/jip-draft.md` in JIP-1's layout, with gas pricing grounded in native timings
measured here.

## 2026-10-23 46 — #32: the JIP's first draft, specification complete, gas constants pending native timings
Why this issue: #32 is next in order, and the specification needs no testnet. Native timings
wait until #23's 2^24 run frees memory, so they don't measure swap.
Did: `docs/jip/jip-draft.md` (JIP-8, cryptographic host calls), in JIP-1's layout:
- six calls with Ethereum precompile encodings: `bn254_g1_msm`, `bn254_pairing_check`,
  `bls12_381_g1_msm`, `bls12_381_pairing_check`, `kzg_point_evaluation`,
  `secp256k1_recover`;
- GP conventions checked against `pvm_invocations.tex`: panic on unreadable memory
  (`export`, 579-583), `HUH` and `OK` codes;
- the motivation, measured on PolkaJam;
- gas as G = ⌈t_native × R × m⌉;
- determinism: a protocol change with an activation slot, unlike JIP-1's optional `log`;
- compatibility, test vectors, reference implementations, four open questions.
Two draft claims were wrong and are fixed: the c-kzg vectors agreed with the code built
natively, and the recompiler's 8–24 ms held in 15 of 30 runs, not "most".
Next: native timings of each call (blst, arkworks, k256, c-kzg) to fill the gas table; then
#32 closes.

## 2026-10-23 47 — #23 closed: Aztec's epoch proof at its real size, 2^24, costs refine 180,791,299 gas on PolkaJam
Why this issue: the 2^24 run finished. #23's row is the cost at 2^24, measured.
Did: `CIRCUIT=rich LOGS=24 spikes/epoch-proof/measure.sh`, 716 blocks of every relation, a
60-minute watchdog (not needed). Meanwhile, `spikes/poseidon2-gas` (#41) was written and
checked on the host, its PolkaJam run still to come.
Evidence:
- "2^24 proof 10592 B, 113 public inputs, prove 127.77 s, peak 7256 MB; bb and our verifier
  accept". 10,592 B = 331 fields, Aztec's `ULTRA_KECCAK_PROOF_LENGTH`.
- PolkaJam: 180,791,299 gas (3.6% of a full package, 18.1% of a tiny one), 0.424 s with the
  machine in swap, verdict 01; a public input altered 9,831,418, 00.
- The every-relation line predicted about 180M.
- `spikes/poseidon2-gas/host` → "p2: 5 permutation vectors, hash_cli's H(7,11,13) and
  H(0,0), and the empty depth-16 root all match".
Updated: RESULTS.md, LESSONS.md, the JIP draft and SERVICE.md now say 180,791,299 (measured)
where they said about 169M or 180M.
Issues: #23 closed.
Next: #41 (Poseidon2 on PolkaJam), #40 (the Solidity verifier in Foundry), then the JIP's
native timings (#32).

## 2026-10-23 48 — #41 closed: a depth-40 Poseidon2 insertion costs 17.1M gas, more than accumulate's 10M; Poseidon2 joins the JIP
Why this issue: the testnet was free, the spike was ready, and its answer settles the JIP's
second open question.
Did: `spikes/poseidon2-gas/measure.sh` on PolkaJam. Recorded in RESULTS.md, LIMITS.md
(candidate 3) and the JIP draft (call 7, `poseidon2_bn254_permute`; open question 2
answered).
Evidence: host "p2: 5 permutation vectors, hash_cli's H(7,11,13) and H(0,0), and the empty
depth-16 root all match". PolkaJam: 1 hash 437,462; 10 chained 4,297,147; 100 chained
42,892,428 (428,839 a hash); 1 depth-40 path 17,515,645; 10 paths 171,853,288 (17,148,627 a
path); verdicts 01, and 00 for a wrong root.
Issues: #41 closed.
Next: #40 (bb 6's Solidity verifier in Foundry), then the JIP's native timings (#32),
including Poseidon2's.

## 2026-10-23 49 — #40 closed: Aztec's epoch verifier costs 721,239 gas on Ethereum, so whole verifiers are about 3× JAM's share, not 50×; #29 closed
Why this issue: #40 was the last missing comparison in LESSONS.md (#29) and the JIP's
motivation, and needed only Foundry.
Did: `spikes/evm-verifier`. bb 6.0.0's Solidity verifiers (`--optimized` as Aztec builds its,
and plain) for #23's every-relation keys at 2^16, 2^20 and 2^24, run in Foundry 1.7.1 on the
same proofs PolkaJam checked. Each test asserts true, and a revert with a public input altered.
Evidence: `forge test -vv` → all 6 PASS; optimized 618,779 (2^16), 669,945 (2^20), 721,239
(2^24); plain 1,632,783, 1,847,625, 2,063,360.
Correction: LESSONS.md, LIMITS.md and the JIP draft said a JAM service pays "about 50 times"
Ethereum's share for cryptography. That holds for checks Ethereum prices as precompiles (KZG
48×, ecrecover 52×). A whole UltraHonk verifier is about 3× (3.6% of a package against 1.2%
of a 60M block), because Ethereum's verifier is contract code too. All three now say so.
Issues: #40 closed. #29 closed: LESSONS.md is complete, every claim linked to evidence.
Next: the JIP's native timings (#32), which fill the gas constants.

## 2026-10-23 50 — #32: native timings; a host call saves about 2–10×, not Ethereum's 50×; #42 filed for the recompiler's R and f
Why this issue: #32's draft lacked only its gas constants, and #33 waits on a complete draft.
Did: `spikes/jip-bench`, native single-threaded timings of every proposed call, and K = PVM gas
per native ns for the three operations whose same Rust code ran on PolkaJam.
Evidence: `cargo run --release` → Poseidon2 9.5 µs (K 44.9); KZG point evaluation 2,989.8 µs
(K 66.7); ecrecover 136.9 µs (K 95.8); BN254 pairing 2/4 pairs 795.9/1,161.0 µs; BN254 MSM
1/16/128 92.5/478.3/2,115.8 µs; BLS12-381 pairing 2 pairs (blst) 772.1 µs; BLS MSM
135.1/337.0/422.5 µs.
Finding: the "about 50×" gap against Ethereum is mostly Ethereum's pricing. Its EVM is slow at
256-bit arithmetic, so precompiles are priced low. The PVM's recompiler runs this code
f ≈ 3.5–10× slower than native (CI's UltraHonk, ledger 26), so a host call priced consistently
saves about 2–10×. Poseidon2 halves (about 200,000 gas), which brings a depth-40 insertion to
about 8M, inside accumulate's 10M. The JIP draft's pricing section and motivation now say so.
Issues: #42 filed (R and f on an x86 recompiler) and added to RALPH's table.
Next: #42 in CI (x86 PolkaJam recompiler, the KZG, ecrecover and Poseidon2 spikes, native on
the same runner); then #32 closes with constants.

## 2026-10-23 51 — #26 closed: the settlement service's skeleton keeps Aztec's proven tip on PolkaJam; #42 rerun after a clean-machine fix
Why this issue: #26 needed only its skeleton (its design and costs were done), and the testnet
was free while CI measured #42.
Did:
- `spikes/settlement`: refine verifies an Aztec-shaped epoch proof (UltraHonk, verifier at
  bb-v6 2ae44db) and outputs previous and end roots; accumulate advances `tip` only from the
  tip.
- Three proofs of #23's 2^16 every-relation circuit with chained inputs (A 1→2, B 2→3,
  C 5→6), each verified by bb.
- #42's first CI run failed: `kzg-gas/measure.sh` never fetched the batch vectors, which I
  had downloaded by hand. It fetches them now (tested from a clean directory: 122, 29, 24),
  and CI was rerun.
- The bb build without its self-check (#22) finished; that fixture is next.
Evidence: `spikes/settlement/measure.sh` → A advanced (tip 2, 170,427,917 gas); A again stale;
C stale; B advanced (tip 3, proven 2); B altered fails in refine (8,330,413, last 00).
Issues: #26 closed.
Next: #42's CI result; #22's invalid-inner fixtures with the unchecked bb.

**Correction to ledger 51:** the bb build without its self-check did not finish. It failed at step
235 of 273: `aztec_constants.hpp` not found. Aztec's monorepo generates that header at configure
time (`barretenberg/cpp/CMakeLists.txt` → `protocol/constants-codegen`), and the sparse checkout
lacked it. `build_bb_unchecked.sh` (ultrahonk_verifier) now checks out the codegen and generates
the header (274 constants). The build is resumed.

## 2026-10-23 52 — #30 closed: JAM can check users' proofs directly, 29 a full package; #42 closed: R ≈ 7–8 gas/ns, f 4.7–9.4 on x86; #32 closed with its constants
Why these issues: #30 lacked only its "proving inside JAM" measurement, which the free testnet
could run while CI measured #42. #32 lacked only #42's constants. #33's reviews wait on #32.
Did:
- `spikes/batch-verify`: n UltraHonk proofs in one refine. `docs/privacy/ARCHITECTURES.md`:
  five candidates. The recommendation is Aztec as it is with JAM as settlement (#28), and for a
  JAM-native service, candidate 3: users' proofs verified in parallel across cores, nullifiers in
  accumulate's storage, one tree package a slot. It says where 3 does not apply: Aztec's CHONK,
  about 12B gas, estimated. Filed #43 for the batch append, which is reasoning so far.
- #42: the jip-pricing workflow passed on x86 (run 40020001416). The JIP's *Gas pricing* now has
  R = 8 and a provisional m = 2. It gives every call's constant, piecewise MSM pricing because a
  single slope overcharges a one-point call 2.5× and 5×, and the saving: 2–4× a call.
  LIMITS.md and jip-bench's RESULTS follow.
- **Correction:** the KZG gas in the docs (199,473,075 for a point evaluation, 262,430,040 for a
  blob, and the extrinsic, export and batch rows) came from an earlier build of the kzg service.
  CI ran the current code from clean: 199,453,527, 262,626,180, …, 967,993,490 for 6 blobs, at
  most 0.1% off. Every file but this ledger now carries the current figures, and kzg-gas's
  RESULTS says where they come from. A local out/ had failed rows (11–15, 17) and was not used.
- The unchecked bb (#22) builds: `ipc-runtime/cpp` was missing from the sparse checkout
  (BB_LITE leaves out the IPC server that `bb`'s entry point links). `bin/bb` runs.
Evidence: batch-verify → 1 proof 170,404,578 (01); 4 → 681,095,607 (01); 8 and 12 out of gas on
tiny (999,999,97x); 4 with the 4th altered 519,006,957 (00). CI: KZG R 7.9, f 6.0; ecrecover
×33 R 6.9, f 9.4; Poseidon2 ×100 R 7.0, f 4.7.
Issues: #30, #42, #32 closed; #43 filed.
Next: #22's invalid-inner fixtures with the unchecked bb; then #33's reviews.

## 2026-10-23 53 — #22 closed: the bb 6.0.0 verifier passes all its tests, invalid-inner fixtures included
Why this issue: the unchecked bb built in 52, and #22 needed only these fixtures.
Did: `generate_test_fixtures.sh` (ultrahonk_verifier) makes the invalid-inner proofs with
`BB_UNCHECKED` (`build_bb_unchecked.sh`) and checks that stock bb rejects them. Every fixture
was regenerated with bb 6.0.0 and nargo rc.3 from aztec-packages v6.0.0. `bb-v6` is pushed at
ae599d3, to abutlabs's fork only.
Evidence: `cargo test --release` → 49 passed, 0 failed, plus 1 doc test. Both invalid-inner cases
fail at "Shplemini pairing check failed". Stock bb rejects both proofs.
Pins: the services stay on 2ae44db. ae599d3 changes no verifier source (`git diff 2ae44db
ae599d3 -- src Cargo.toml` is empty), and a rebuild at the new rev gives a blob 7 bytes shorter,
which would leave the gas measured at 2ae44db (#23, #26, #30) unmatched to the code.
Issues: #22 closed.
Next: #33, the simulated reviews of the JIP.

## 2026-10-23 54 — #43 closed: one refine appends 11,350 notes to a depth-40 tree; a JAM-native tree is bounded by verification, not insertion
Why this issue: #30's recommendation rested on a reasoned insertion rate. This measures it,
while the testnet was free and #33's reviewers work.
Did: kind 2 in `spikes/poseidon2-gas` appends k notes level by level from a depth-40 tree's
frontier. The expected roots come from `tools/append.py`'s per-leaf insertion, which it holds
to a naive tree at depth 4, along with the level-by-level algorithm. ARCHITECTURES, LIMITS
and LESSONS now use the measurement: one tree package a slot takes about 5,675 transfers, and
the limit to one package a slot is this demo's design, not the spec. The rebuilt service moved
the old cases by 2 gas a hash (428,841); the docs carry it, and the CI R and f table keeps its
run's record.
Evidence: `measure.sh` on PolkaJam → append 1: 17,917,418 (01); 64: 46,380,829 (01); 1,024:
466,117,396 (01); 2,048: 915,618,413 (01); 2,048 with a wrong root: 915,618,411 (00). That is
438,966 gas a note.
For #34: jip-draft.md's Poseidon2 motivation ("without it, every tree update must be proven
and applied in order, one package a slot") is now wrong. The final corrects it.
Issues: #43 closed.
Next: #38's control run; #33's cross-responses when the reviews land.

## 2026-10-23 55 — #38 closed: imports by package hash follow the GP's 8-block recent history; PolkaJam is right
Why this issue: it needed only the testnet, free between #43 and #33's cross-responses.
Did: `spikes/kzg-gas/tagged-import.sh` exports a blob's 32 segments, imports them by package
hash at once, again after 11 slots, and by segments root after the same wait. The JIP-3
telemetry names the failure. kzg-gas's RESULTS and BLOBS.md record the cause and the rule
for builders. The export overhead is corrected to 117,465 at the current code.
Evidence: package 21 (by hash, 1 slot after) was reported with verdict 01 from 3 refines.
Package 22 (by hash, 13 slots after) got "Not reported in time", with both secondaries logging
`WorkPackageFailed { reason: "Segment-root lookup is invalid" }`. Package 23 (by root) was
reported with 01. GP 0.8.0 `reporting_assurance.tex:401-423` and H = 8
(`definitions.tex:269`).
Issues: #38 closed.
Next: #33's cross-responses (Gavin Wood's review is in; Zac Williamson's is pending).

## 2026-10-23 56 — #33 closed: two simulated reviews of JIP-8, cross-responses and a synthesis; the demo still passes on pj6 from clean
Why this issue: #32's draft was complete, and #34 waits on the reviews.
Did: two agents, one per persona, each reviewed jip-draft.md at 4f0fd19 independently, then
answered the other's review. Each file opens with the simulated-review disclaimer.
`docs/jip/reviews/synthesis.md` lists the ten revisions both reviewers support, the choice on
each of seven disagreements, and the disposition of all 28 comments. New issues: #44 (UltraHonk
gain, running), #45 (end-to-end benchmark), #46 (CHONK with amortised IPA), #47 (the ISA
alternative). ZW-14's stale 169M and 369M are fixed in SERVICE.md and kzg-gas's RESULTS.
Evidence:
- GW: 14 comments (2 blocking, 9 major, 3 minor); responding to ZW, agree 8, in part 5,
  disagree 1.
- ZW: 14 comments (1 blocking, 7 major, 6 minor); responding to GW, agree 8, in part 6,
  disagree 0.
- Phase 3's demo condition: `NET=pj6 nets/demo.sh down; up; check` → every assertion ok, PASS,
  exit 0.
Issues: #33 closed; #44–#47 filed.
Next: #44's measurement, #45's benchmark, then #34's final JIP.

## 2026-10-23 57 — #44 closed: an UltraHonk verification is 77% curve work, 23% the rest; with the core calls about 2.2–3.3× cheaper; 14% is free today
Why this issue: #34's motivation needs it (GW-10, ZW-1), and the testnet was free.
Did: `spikes/verifier-profile` verifies #23's every-relation proofs at 2^16, 2^20 and 2^24 with
the default hooks and with `Gauge`, which converts points field by field and meters each hook
with the gas host call. It projects the gas with calls 101–102 under the draft's pricing and
under the GP's sizing. Filed #48 to use such hooks in the services.
Evidence: at 2^24, `()` 181,113,874 and `Gauge` 155,924,836, both verdict 01. Gauge's split: MSM
71,158,380 (62 points), Miller loop 29,621,556, final exponentiation 19,051,852, rest
35,819,314. Projected 81.4M (R = 8) or 54.8M (R = 3.33), m = 2 in both.
Issues: #44 closed; #48 filed.
Next: #45's end-to-end benchmark, then #34.

## 2026-10-23 58 — #45 closed: the core calls timed end to end on x86, priced in the GP's unit; an UltraHonk check would be 3.5× cheaper, each call 7–13×
Why this issue: both simulated reviewers made the bare-kernel constants blocking (GW-3, ZW-10),
and #34 needs constants.
Did: `spikes/jip-bench` e2e times calls 101–104 from their EIP-encoded input, with every check.
It uses random and adversarial inputs and two implementations a curve, and reports median, p99
and max of 101 runs. Its blst is now `no-threads`: the first timings had used blst's default
thread pool. `e2e-pricing.py` prices at 10/3 gas/ns × m = 2, the GP's sizing; the jip-e2e
workflow runs it on x86. `e2e --vectors` writes 74 JAM-native vectors. The profile service
runs calls 101–102 as PVM code, so each call has a PVM baseline. #44's rows are re-measured
with the rebuilt service (within 0.1%).
Evidence:
- CI run 40026200133 (EPYC 9V45): the implementations agree on all 144 inputs. BN254 pairing
  is 4,196,895 + 2,030,652 k; BLS12-381 pairing 1,827,334 + 1,727,240 k. MSMs are tabulated:
  BN254 at 128 points 12,852,234, BLS12-381 66,119,300.
- PolkaJam: BN254 MSM at 128 points costs 110,727,638 as PVM code, and a 2-pair check
  64,337,004. That is 7.2–13.3× the calls' gas. A 2^24 UltraHonk check projects to 52.1M,
  against 181.1M (3.5×).
Issues: #45 closed.
Next: #34, the final JIP.

## 2026-10-23 59 — #34 closed: JIP-8 final, four curve calls priced in the GP's unit, with a change log and a submission note; not submitted
Why this issue: #33's reviews, #44's gain and #45's constants were all in.
Did: `docs/jip/jip-final.md` makes the agreed revisions. The JIP has four calls (101–104: BN254
and BLS12-381 G1 MSM and pairing check), the EIPs' semantics with an `OK`/`HUH` mapping, and an
evaluation order with gas from φ8. Its invariant is R_GP = 10/3 gas/ns × m = 2, with an MSM
table and pairing bounds from #45's x86 run. Activation is keyed to the lookup-anchor slot, with
a probe and reserved indices, and Ψ_I is excluded. It adds Rationale, Alternatives considered, a
KZG composition, Process, and a change log citing the comments. `docs/jip/SUBMISSION.md` lists
what is ready and six steps before submitting. LIMITS, LESSONS and ARCHITECTURES follow the final
JIP, and the draft points to it.
Evidence: `e2e --vectors | cmp - docs/jip/vectors/jip8-core.jsonl` gives no output. `e2e-pricing.py
< ci/e2e-40026200133.txt` reproduces the constants. The UltraHonk projection (52,122,624 at
2^24) is in `spikes/verifier-profile/RESULTS.md`.
Issues: #34 closed. Open: #28 and #35 (tracking), and #46, #47 and #48 (research, not phase-3 rows).
Next: #28 and #35's status; #48; Phase 3's done check.

## 2026-10-23 60 — Phase 3's done check
Why: every Phase 3 row's issue is closed, and #28 and #35 carry end-of-phase status comments.
Checked:
- **Rows:** #21–#27, #29–#34, #36–#42, #44 and #45 are closed. Each closing comment gives its
  command and output, and each has a ledger entry (31–59). #28 and #35 stay open as `roadmap`,
  with status comments listing what is done and what remains.
- **Demo:** `NET=pj6 nets/demo.sh down; up; check` passed from clean in ledger 56 (PASS, exit
  0). `git diff --stat 5f3439a..HEAD -- services client nets` is empty, so the code it ran is the
  code now.
- **CI:** green on this commit (below, once its run finishes).
Left open as research for later phases, not Phase 3 rows: #46 (CHONK with an amortised IPA),
#47 (the instruction-level alternative), and #48 (field-wise hooks in the services, which change
soaked code and so need a re-soak).
Nothing was submitted or posted outside this repository's issues.
- **CI, recorded:** run 40027377792 on 32745d9 passed (the ci workflow). The commit adding this
  line touches only this file, and its run is checked before the phase is called done.

## 2026-10-24 61 — Phase 4 opened: Aztec on JAM without the JIP
Why: Aodh asked to complete Aztec on JAM under today's specs, keeping JIP-8 in reserve.
Did: RALPH.md's Phase 4 section, and issues #49–#53 under #28. The lasair fuzzer check gives
exit 0 (nothing new). Aztec v6's L1 fixtures (`mixed_checkpoint_1.json` and others) hold real
headers, header hashes, blob commitments, batched blob inputs, bodies and inbox messages. So
the settlement service can be tested on Aztec's own encodings before any node runs.
Machine: 16 GB of RAM, Docker's VM 8 GB, 11 GB of disk free. #52 and #53 will need room for
Aztec's packages.
Next: #49, the checkpoint path. It comes first because #50 and #51 build on its stored
checkpoints.

## 2026-10-24 62 — #49 closed: the settlement service's checkpoint path, on Aztec's own encodings, on PolkaJam
Why this issue: #50 and #51 build on its stored checkpoints.
Did:
- `crates/aztec-l1` ports v6's `Hash.sha256ToField`, `ProposedHeaderLib.hash` (348 packed bytes),
  `BlobLib`'s blob hash, blobs hash and blob-commitments accumulator, and `validateHeader`'s
  stateless checks.
- `services/aztec-settlement` is the propose path: refine checks the header and blobs and
  exports the blobs; accumulate chains on the tip with JAM's clock. Its config is compiled in
  (build.rs: jam080-build clears the environment).
- `tools/aztec-gen` makes checkpoints with Aztec's released v6 packages (`@aztec-labs/stdlib`,
  `blob-lib` 6.0.0).
- The KZG crate moved to `crates/kzg-verify`. The privacy service builds to the same code hash
  before and after the workspace change (0x97e7677f…).
- CI builds the service and runs the crate tests.
Found:
- Aztec's fixtures carry no blob data, so they test the encodings natively, and the generated
  checkpoints test the blobs.
- A refused item must still export its declared segment count, or the GP replaces its output
  with an error (`work_packages_and_reports.tex:215`), so refine exports zeros.
Evidence:
- `cargo test -p aztec-l1`: 4 passed. The header hash and blobs hash equal all six fixtures'.
- `measure.sh`: all eight cases as expected (accepted 1, 1, 1; refused 10, 10, 2, 3, 13).
  `cp:1–3` hold Aztec's header hashes, and the tip is 3. Gas: 262.4M for one blob, 404.6M for
  two.
Issues: #49 closed; #54 filed (signatures).
Next: #50, the Inbox.

## 2026-10-24 63 — #50 closed: the Inbox, messages from JAM services in Aztec's rolling-hash buckets
Why this issue: the epoch path (#51) checks the inbox rolling hashes, so it needs them.
Did:
- `aztec-l1`: `l1_to_l2_leaf` and `inbox_rolling_hash`, tested against Aztec's TypeScript
  (`tools/aztec-gen/inbox.mjs`, `@aztec-labs/stdlib` 6.0.0).
- The service absorbs deferred transfers as messages, one bucket a JAM timeslot (at most 256
  a bucket), and stores the messages for the node. A checkpoint now names its bucket, and
  accumulate checks consumption as `validateInboxConsumption` does. cfg gains chain id and
  version, and a compile-time length check (a stale 104-byte cfg had built a 3.7 KB blob with
  everything compiled out).
- `spikes/inbox-sender` stands in for an L1 contract. `testlib.py` is shared by both tests, and
  `gen.mjs` takes a spec (names, parents, slots, inbox hashes).
Found: the fixtures' inbox hash isn't reproducible (no rollup version recorded; Aztec's own
Solidity tests overwrite it), so the reference is Aztec's released code. Also, one of my own
script bugs: reading the wrong service id after the second deploy.
Evidence:
- `inbox.sh`: both buckets' rolling hashes and all 4 leaves equal Aztec's, and 1 message was
  refused (version 2). Checkpoint verdicts were 20, 1, 18, 15, 16, 1 as expected, with cp:1–2
  consuming 4 messages.
- `measure.sh` (#49) re-run on this code: all 8 as before, and Aztec's header hashes stored.
- `cargo test -p aztec-l1`: 5 passed.
Issues: #50 closed.
Next: #51, the epoch path.

## 2026-10-24 64 — #51 closed: the epoch path, Aztec's 113 public inputs, proved by bb 6 and settled on PolkaJam
Why this issue: after #49 and #50 every value it compares is stored.
Did:
- The service's kind 2: refine verifies the root rollup proof (zkVerify's verifier, bb-v6
  2ae44db) and the batched blob opening, and checks the last six public inputs against the
  opening. Accumulate runs `assertAcceptable` and compares the other 107 with stored
  checkpoints and cfg, then writes `tip:proven`, `outbox:<epoch>` and the prover.
- cfg is 272 bytes (vk hash, vk tree root, protocol contracts hash, proof window). The
  checkpoint log keeps coinbase and fees.
- `aztec-l1`: the public-input layout and blob limbs.
- `tools/aztec-gen/epoch.mjs` builds the public inputs and opening with Aztec's
  `RootRollupPublicInputs` and `BatchedBlobAccumulator`.
- `testdata/epoch-circuit`: 113 free public inputs at 2^16.
Found: refine prefixed every output with kind 1, so accumulate read an epoch's start byte as
its verdict. The first run showed "accepted" with no proven tip. Fixed: the prefix is the item's
kind.
Evidence:
- `epoch.sh`: the seven verdicts were 1, 1, 36, 4, 5, 1, 37 as expected. The proven tip is
  checkpoint 2 with b's archive, the Outbox holds b's out hash over 2 checkpoints, and cp:2's
  blob commitments hash equals Aztec's `BatchedBlob`'s.
- Refine gas: 359.3M for an epoch proof.
- `measure.sh` and `inbox.sh` re-run on this code: all as before.
Issues: #51 closed.
Next: #52 (node integration). It needs Aztec's local network running, which is disk- and
memory-bound here, so it gets assessed first.

## 2026-10-24 65 — #52 closed: an unchanged Aztec v6 node's L1 needs, recorded; the integration is an Ethereum JSON-RPC facade
Why this issue: #53's path depends on it.
Did: `tools/aztec-local` runs Aztec v6's released node (`@aztec-labs/aztec` 6.0.0, 1.1 GB) as
its local network, with Foundry 1.8.5 (Aztec's pin) in its own directory, since the global
1.7.1 lacks a flag Aztec passes. `rpc-proxy.mjs` records every L1 request, and `analyze.mjs`
names each one by Aztec's addresses and ABIs. `docs/aztec/NODE.md` records the result and the
decision, and `docs/aztec/node-l1-record.json` holds the analysis.
Found:
- **The surface:** 34 methods, 42 view functions (nearly all on the Rollup), and one write
  shape per checkpoint: a blob transaction to Multicall3 wrapping `propose`. The archiver reads
  it back through the transaction and its trace. In steady state, 405 block requests and 286
  calls a minute.
- **Cheat-code proving:** the local network proves by anvil cheat codes.
- **No L1 blobs:** its blobs are in-process.
Decision: a facade backed by the settlement service (JIP-2 reads and submissions). Aztec's
packages run as released. The seams stay a proposal to Aztec.
Coordination: another session (aiden-9f) shares this tree. My commit 7e4d19f had swept in its
docs/TOOLS.md, so I now stage exact paths and we agreed ports.
Issues: #52 closed.
Next: #53, the facade, starting with reads, blocks and logs.

## 2026-10-24 66 — #53, step one: Aztec v6's released node runs on JAM, through the facade
Why this issue: #52 chose the facade, and #49–#51 gave it state to report.
Did:
- `tools/aztec-local/jam-l1.mjs` (the facade): JIP-2 head tracking as L1 blocks, about 60 view
  handlers over the service's storage and cfg (zero values of the right ABI shape, logged,
  for the rest), the propose relay through `jamboree rpc submit` with blob proofs recomputed,
  and synthesized logs, receipts and traces. It also emits the deployment `OwnershipTransferred`
  event, which the archiver probes, and answers `isEpochSetUp`'s raw storage read.
- `services/aztec-settlement/devnet.sh` keeps a PolkaJam testnet up with the service
  configured for the node's genesis.
- `tools/aztec-local/on-jam.sh` starts the devnet, the facade and the node.
Found, in turn:
- The node's genesis under `--node` with test accounts is archive 0x0189bb92…, not the local
  network's: the node entered standby until cfg matched.
- The archiver requires the Rollup's deployment log.
- `getCheckpoint`'s `CheckpointLog` shape.
- `PROVER_REAL_PROOFS=false` puts the node in test mode, as the local network runs.
Evidence: node.log shows "Aztec Node started on chain 0x7a69 …", "Aztec Server listening on
port 8081". `node_isReady` returns true, `node_getBlockNumber` 0, and the facade logged no errors.
Next: step two, `--sequencer` and a transaction.

## 2026-10-24 67 — #53 closed: an unchanged Aztec v6 node settles transactions on JAM
Why this issue: Phase 4's goal.
Did:
- **The facade's sequencer path:**
  - `canProposeAtTime`, `validateHeaderWithAttestations` and
    `validateCheckpointHeaderAndInbox`, applying the service's own checks;
  - `eth_simulateV1` with state and time overrides, scoped per call by `AsyncLocalStorage`;
  - in-flight checkpoints for pipelined simulations;
  - reverts as error 3 with Aztec's custom errors ABI-encoded;
  - nonces counted as a node counts them, and replacement transactions;
  - `CheckpointProposed` carrying the payload digest and attestations hash, computed by Aztec's
    functions.
- `on-jam.sh` gained the sequencer's keys (the standard test mnemonic's first account, as the
  local network uses).
- `send-tx.mjs` sends through Aztec's PXE.
Evidence: two PXE transactions, in L2 blocks 4 and 5, went into checkpoint 4. The settlement
service accepted it on PolkaJam and Aztec's archiver downloaded it with 2 tx. `node_getTxReceipt`
gives `checkpointed` for both. The run had 7 checkpoints, 0 refused, 7 replacements, and no
facade errors.
Not yet: test-mode client proofs and no prover (as the local network runs), no committee (#54),
and P2P off. Written in NODE.md.
Issues: #53 closed.
Next: #48 for the settlement service (14% less gas a proof), then #54 (signatures).

## 2026-10-24 68 — #48 closed for the settlement service: field-wise curve hooks, 18.7M less an epoch proof; the privacy service's switch is #55
Why this issue: it lowers settlement's largest per-package cost, with no JIP.
Did:
- `crates/curve-hooks`: zkVerify's `CurveHooks` on upstream arkworks, converting points field by
  field. The settlement service's epoch refine verifies with it.
- #48 is rescoped in RALPH to the settlement service. The privacy service needs a re-soak, so
  its switch is #55. #54 now has a Phase 4 row.
Evidence:
- `epoch.sh`: the epoch proof's refine is 340,601,756 gas (359,321,493 before), and all 7
  verdicts are as before. The proven tip and Outbox are right, and the blob commitments hash
  equals Aztec's.
- `measure.sh` gave 8 of 8 verdicts and `inbox.sh` 6 of 6, with Aztec's hashes matched.
- The privacy service's code hash is unchanged (0x97e7677f…).
Issues: #48 closed; #55 filed.
Next: #54, the proposer's signature.

## 2026-10-24 69 — #54 closed: the committee's signatures, checked in refine as the Rollup checks them
Why this issue: `ProposeLib.propose` verifies the proposer, the last part of the Rollup not yet
ported.
Did:
- `aztec-l1::signatures`: EIP-712 in the "Aztec Rollup" domain, `ProposeLib.digest`,
  `AttestationLib` (packing, the attestations-and-signers digest, reconstruction), the committee
  commitment, the proposer index, and OpenZeppelin's strict `recover`. It is tested against a
  vector from Aztec's code (`tools/aztec-local/sign-vectors.mjs`).
- The service: cfg is 645 bytes (rollup address, seed, committee of up to 16), the checkpoint
  payload carries the attestations, and refine checks them before the blobs.
- The facade relays `propose`'s attestations, signers and signature. `gen.mjs` signs with
  Aztec's code. `testlib.py` and `devnet.sh` follow.
Found: Aztec's `getHashedSignaturePayload` is the untyped hash; the Rollup checks
`getHashedSignaturePayloadTypedData`'s. My first vector used the wrong one, and the native test
caught it.
Evidence:
- `signatures.sh`: verdicts 1, 6, 6, 6, 1 as expected. Signed checkpoints cost 289.4M, and a
  forgery at most 27.2M.
- `cargo test -p aztec-l1`: 7 passed.
- Re-run on this code:
  - `measure.sh`: 8 of 8, with Aztec's header hashes. It was made robust to a checkpoint's
    random blob count.
  - `inbox.sh`: 6 of 6.
  - `epoch.sh`: 7 of 7.
  - `on-jam.sh` with `send-tx.mjs`: checkpoint 4 holds 2 transactions, with 6 accepted, 0
    refused and 0 errors.
Issues: #54 closed.
Next: Phase 4's done check.

## 2026-10-24 70 — Phase 4: every row closed; the demo check waits on Aodh's demo net
Checked:
- **Rows:** #48–#54 are closed, each with its command and output on the issue and its ledger
  entry (62–69). #28 carries an end-of-phase status comment.
- **CI:** green on 3354962 (run 40073720562).
- **Demo:** not yet re-run. The committed demo code (services/privacy, client) is unchanged since
  Phase 3's pass, and the privacy service builds to the same code hash, 0x97e7677f…. But the
  done condition asks for a run from clean. The other session (aiden-9f) has its polkajam demo
  net up for Aodh's browser, and pj6 can't run beside it: indexer port 19700, the shared
  out/state ids, out/wallet.pid, and the indexer container. Once Aodh is done with it: `NET=polkajam
  nets/demo.sh down`, `NET=pj6 nets/demo.sh down; up; check`, then `NET=polkajam nets/demo.sh up`
  to restore his net.
