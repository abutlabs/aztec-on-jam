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
