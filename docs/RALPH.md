# The build loop

This is the brief a Ralph loop reads at the start of every iteration. The same text each
time; the state is in the files and the git history. Read, in this order:

1. `docs/LEDGER.md` — what earlier iterations did, with evidence, and what is next.
2. `docs/PLAN.md` — the steps and gates. The ledger overrides the plan where they differ.
3. The code.

Then do the next step the ledger names. One step an iteration, verified, committed,
pushed, ledgered. Don't start a second step in the same iteration, with one exception: while
a step waits on a long run (a soak, a clean-machine build), an independent step that does
not touch what the run uses may proceed; each step gets its own ledger entry.

## Where things are

- `/Users/austinkloske/Projects/abutlabs/Aiden/submodules/jam-privacy-service` — this
  repo: the service (`services/privacy`), the public-balance helper, spikes, nets, docs.
  Push to `origin main` after every verified step.
- `../aztec-packages` (fork, pinned at `v6.0.0`), `../aztec-node` (fork), `../demo-wallet`
  (fork). Work on a branch named `jam` in each; push the branch to `origin` (abutlabs).
  Never rebase onto upstream inside the loop.
- `../lasair` (the JAM client, OCaml) and `../jamswap` (the tiny nets, the deploy and
  submit tooling in `offchain/`). Use them through their documented interfaces:
  `jamswap/docker-compose.lasair6.yml`, `jamswap/nets/compose/pj6.yml`,
  `jamswap/offchain/deploy.py`, `jamswap/offchain/chain.py`, lasair
  `scripts/keystore-net.sh` (six lasair validators as host processes, no Docker). A change
  inside lasair or jamswap is allowed only when nothing else works, kept minimal, on a
  branch `jam-privacy`, and ledgered with the reason.
- `../zk-jam-service` — the pattern to copy for a refine verifier, a spike's gas
  measurement (`spikes/groth16-gas`) and a lasair deploy.
- Never commit or push in the parent `Aiden` repo.

## The machine

macOS, 10 cores, 16 GB, about 20 GB of disk free. Check `docker info` once; if Docker
is absent or not running, the tiny net is lasair's `scripts/keystore-net.sh` and
PolkaJam is out of reach, which the ledger must say. Mind the disk: no full clones, keep
`node_modules` to the packages the step needs, `cargo clean` spikes you are done with.
Install tools the step needs (`aztec-up` for `aztec-nargo` and `bb`, `cargo install
jam-pvm-build` or `cargo-jam-build`), pin the versions in the ledger, and add each one
to `docs/TOOLS.md` with its install line.

## What demo ready means

`<promise>DEMO READY</promise>` may be written only when all of these are true and the
ledger carries the command and output for each:

1. `nets/demo.sh up` starts a six-validator tiny JAM net, deploys the privacy service and
   the public-balance service, and starts the indexer, the builder and the wallet, from a
   clean machine state, in one command. `nets/demo.sh down` removes it all.
2. `nets/demo.sh check` runs, without a browser, for two accounts: shield 100 from account
   A, A sends 40 privately to B, B unshields 40. It then reads the chain and asserts, byte
   for byte: the note root changed three times and equals the builder's; exactly the
   expected nullifiers are in storage; the public-balance service shows A at minus 100 and
   B at plus 40; a replay of the send package is dropped. It exits 0 only if every
   assertion holds, and prints each one.
3. The proofs are real: made by Barretenberg from Noir circuits on the sending side,
   verified in `refine` on the PVM, with the gas of one verification recorded in the
   ledger and in `README.md`.
4. The wallet (the `demo-wallet` fork) does the same three moves in a browser against the
   same net, for a second person to see. A screenshot of each move is in `docs/images/`.
5. `README.md` has a *Demo* section whose stage commands are one word each
   (`up`, `check`, `shield`, `send`, `unshield`, `down`), and a *What is real and what is
   reduced* section (below).
6. `docs/LEDGER.md` ends with the verification run, dated, copied from the terminal.

## Liberties, and their price

Take any shortcut that gets to a real end-to-end demo sooner, as long as the ledger and
the README's *What is real and what is reduced* section record it and name the way back:

- **Circuits.** If Aztec's kernel circuits can't be anchored to a JAM header in a few
  iterations, write one Noir transfer circuit (notes in, notes out, nullifiers, Merkle
  membership against the root) and ship with that. Aztec.nr and the kernels come back
  later; say so.
- **Verifier.** Run step 0 of the plan first. Pick the decision row the measurement gives.
  A Groth16 wrap of the folded proof is a fine first verifier; UltraHonk on the PVM can
  come later. Record gas for whatever runs.
- **Folding.** One proof per package is allowed for the demo (no folding) if the gas
  allows one verification per package and the check still passes.
- **PXE.** If the Aztec PXE's node adapter is too large, the first client may be a small
  TypeScript module using `bb.js` and the circuit artifact directly; keep the note and
  nullifier formats Aztec's so the PXE can replace it.
- **Indexer.** Reading service storage over JIP-2 from a script is enough at first.
- **Public balance.** A ten-line service holding `account -> u64` is enough.
- **Net.** `lasair6` under Docker, or `keystore-net.sh` without. `pj6` is a stretch goal
  if Docker and the PolkaJam image are available; ledger the result either way.

What may not be reduced: real proofs verified in refine, nullifiers and roots in
accumulate, shield and unshield as deferred transfers, the byte-exact check, honesty in
the ledger.

## How to work

- Measure before deciding; the gate is the gate.
- Build the smallest thing that moves the check forward, run it, read the output, then
  commit. A step with no run is not done.
- No duplicated code between the two services; share a crate.
- When something fails three times the same way, stop, write the failure and the
  hypotheses in the ledger, and take the next step that doesn't depend on it.
- Write for Aodh: short, specific, with the command and the numbers.

## GitHub issues: the work is visible there

The repo `abutlabs/jam-privacy-service` tracks everything as issues, so Aodh can follow
and so later sessions can pick up where this one stopped:

- One `milestone` issue per milestone, one `step` issue per plan step (they exist; `gh
  issue list -R abutlabs/jam-privacy-service`). A new piece of work gets a new issue
  before it starts; a finding that decides something goes on a `research` issue as a
  comment with the evidence; a choice made under *Liberties* gets a `decision` issue
  naming its price and the way back; anything that stops a step gets a `blocker` issue.
- Every measurement gets a `benchmark` issue (what ran, on what, the numbers), and later
  runs of the same measurement go on it as comments, so the numbers have a history. The
  lasair-against-PolkaJam comparison has one already.
- Every iteration comments on the issue it worked, with the command and output, and
  closes the issue when its check passes. Commits name the issue (`#12`).
- The ledger stays the sequential record; the issues are the map. Both, every iteration.

## The ledger entry

Append to `docs/LEDGER.md` at the end of every iteration:

```
## <date> <iteration> — <step name>
Did: …
Evidence: `<command>` → <the lines that prove it>
Next: <one step>
Blockers: <none, or what and why>
```

---

# Phase 2: ready for the JAM implementers' channel

Phase 1 ended with `DEMO READY` (ledger entry 9). Phase 2's goal, from Aodh (2026-10-23):
close every open issue and make the demo polished enough to show the JAM implementers'
channel, with the testnet improvements included. Everything phase 1 made true must stay
true: `nets/demo.sh up`, `check` and the e2e run pass on every net the demo supports.

The issues open when phase 2 began, and what closes each one:

| Issue | Closed when |
| --- | --- |
| #19 the wallet holds every key | The browser holds its own key and proves in the page (noir_js + bb.js); notes reach the receiver through the chain, encrypted; two browser contexts are two wallets that share nothing but the chain; `client/wallet.py` is gone or only a static server. The e2e run proves it. |
| #18 our circuit, not Aztec's kernels | The demo-visible reductions are gone: encrypted notes on chain with tags (shared with #19), nullifier non-membership proven in the circuit (an indexed nullifier tree, Aztec's shape) with accumulate checking that tree's root, and the note tree at Aztec's depth or a measured reason not to. Plus a spike that compiles and proves Aztec v6's private kernel circuits with our toolchain and says, with numbers, what a kernel-based port would take. If that port is beyond a loop, it becomes one `roadmap` issue, named in the final report. |
| #15 bb 3.0.3 pin | The verifier accepts the proof layout of the bb that current Noir pins (bb 5.x for Noir rc.5), in a fork of zkVerify's verifier, measured on the PVM, and the toolchain moves to it; or a measured reason to stay, with both layouts tested. |
| #11 which verifier | The verifier decision is final after #15; a closing comment says so. |
| #13 verifier gas | The 312M (spike) against 182M (service) difference is explained by measurement, and the final table (layouts, zk and plain, public-input count) is in the README. |
| #12 tools | `scripts/bootstrap.sh` checks and installs every tool at its pinned version on a fresh macOS or Linux machine, and `docs/TOOLS.md` matches it. |
| #14 lasair vs PolkaJam | The same privacy package refined and accumulated on both clients: refine wall time, slot-to-accumulation latency, move latency in the wallet, on lasair6, pj6 and a mixed net; a table in the README and the issue. |
| #4 an hour on pj6 | `nets/demo.sh` runs on the PolkaJam-only net, and a soak sends one privacy package a slot for an hour: zero dropped roots, finality advancing, numbers in the ledger. |
| #17 testnet plumbing | Our fork of jamixir's jamboree (`../jamboree`, branch `lasair`) launches the demo's nets (lasair-only, PolkaJam-only, mixed) and has `deploy`, `submit`, `read` and `status` over JIP-2 in Rust; the demo uses it instead of jamswap's Python; its headless check runs in our verification; and an issue on jamixir/jamboree offers the work upstream (Aodh asked for this), linking the branch. |

Also, for the channel:

- **lasair#93** (packages Ready on JIP-2 that never land): investigate in `../lasair` on a
  branch, fix it with lasair's own gate passing, and run the demo on a locally built image.
  If it cannot be fixed safely, say why in the issue and the README.
- **Polish:** a recorded walk-through (a GIF or video of the two browsers, made by the e2e
  script), `docs/DEMO.md` for an implementer who has ten minutes, the README tightened, and
  `docs/ANNOUNCE.md`, a short post for the channel that Aodh sends himself. Never post to
  the channel or any public forum from the loop, apart from the jamboree upstream issue.
- **CI:** a GitHub Actions workflow that builds both services and the circuit and runs the
  checks that need no network (`cargo check`, `nargo test`, the verifier spike's host test).

## Phase 2 done

`<promise>CHANNEL READY</promise>` may be written only when all of these hold, with the
evidence in the ledger:

1. Every issue in the table above is closed, each with a closing comment that links its
   evidence. The only issues left open are `roadmap` issues opened under the #18 rule, and
   the final report names each one.
2. lasair#93 is closed with a fix, or carries a comment explaining why it cannot be fixed
   safely, and the README says which.
3. A final verification run, copied into the ledger: `down`, `up`, `check`, the stage
   commands and the e2e run, on lasair6 and on pj6, from a clean state, all passing.
4. `docs/DEMO.md`, `docs/ANNOUNCE.md`, the recording and the CI workflow exist, and the CI
   run on the last commit is green.

---

# Phase 3: Aztec on JAM, privacy on JAM, and a JIP

Phase 2 ended with `CHANNEL READY` (ledger 27, corrected in 28; the demo's checks still hold
and must keep holding). Phase 3's goal, from Aodh (2026-10-23): run Aztec's own software on
JAM with JAM as its settlement layer (#28), and answer his privacy brief (#35): what this
work teaches about privacy on JAM, whether another architecture suits JAM better, and, if
the current specs have a fundamental limit, a JIP, reviewed and finished, ready for him to
submit later.

## Every iteration

1. Read the ledger's last entry, then `gh issue list -R abutlabs/jam-privacy-service`.
2. Review the open issues: pick the next unblocked one by the order below (a dependency
   first; then the one whose result most changes the others). Say in the ledger why that
   one.
3. Do one step on it, verified with a real run or a cited source; commit, push, comment on
   the issue with the command and output; close it only when its row below holds, on
   evidence from the code as it now is (re-run anything a later change touched).
4. A new question or direction found on the way gets its own issue before work on it starts.

## Rules for this phase

- **Measure Aztec and ZK work on PolkaJam**, never lasair (Aodh): the native
  `polkajam-testnet` v0.8.0 as `spikes/epoch-proof/measure.sh` runs it, gas and time from
  PolkaJam's JIP-3 telemetry, verdicts read back from service storage.
- **This machine has 16 GB.** A 2^24 proof does not fit (#23); don't retry what swapped.
  Prefer a smaller circuit made of heavy opcodes, or say what bigger machine it needs.
- **Personas are simulated.** The JIP reviewers write as personas of Gavin Wood and Zac
  Williamson for review quality; every review file says at its top that it is not the
  person's view. Never attribute a statement to the real people.
- **Nothing is submitted or posted**: no JIP submission, no upstream issue or PR to Aztec,
  zkVerify or the JAM repos, unless Aodh asks in a later message. Drafts live in this repo.
- **Sources for every claim** about the Gray Paper (0.8.0 text, section), Aztec (code path
  at a pinned commit) or history (zk.money): cited inline.

## The issues, and what closes each

| Issue | Closed when |
| --- | --- |
| #22 verifier for bb 6.0.0 | All crate tests pass, including invalid-inner fixtures made with a bb 6.0.0 built without its self-check (or a recorded reason they cannot be made); the `bb-v6` branch pushed; the services that use it pinned to its commit. |
| #23 epoch proof in refine, on PolkaJam | The cost at 2^24 is measured (a heavy-opcode circuit, or a real root rollup proof with its key), or the largest feasible size plus a recorded reason; the table in `spikes/epoch-proof/RESULTS.md`. |
| #24 blob commitments | The gas of one EIP-4844 KZG point evaluation in refine on PolkaJam, measured; where blob data lives on JAM, decided with reasons. |
| #25 the node's Ethereum boundary | `docs/aztec/BOUNDARY.md`: every touchpoint with its interface status, the seams Aztec would need (as a drafted, unsent upstream proposal), and sizes. |
| #26 the settlement service | `docs/aztec/SERVICE.md`: refine/accumulate split, proposals and signers, the Ethereum-supplied values and JAM's answers; a skeleton service that checks a proof and keeps a tip, deployed on PolkaJam. |
| #27 tracking Aztec's releases | `docs/aztec/RELEASES.md`: the v5 → v6 diff across the four boundary items. |
| #36 PolkaJam's package cap | The threshold pinned by probing sizes, where it applies (payload, item or package; RPC or CE-133) as far as a binary-only client lets us see, and recorded as PolkaJam behaviour against the Gray Paper's W_B. |
| #37 blob export path and batching | On PolkaJam: a blob exported as segments by refine and imported by a later package (gas, time), and batch verification against single checks. |
| #38 PolkaJam's tagged imports | The cause located as far as a binary-only client allows (the CE-134 share's segments-root mappings, or the secondaries' history), recorded with the workaround (import by segments root). |
| #39 ECDSA in refine | One and 33 ecrecovers measured on PolkaJam, after the implementation agrees with geth's precompile vectors. |
| #40 UltraHonk verifier gas on Ethereum | bb 6's Solidity verifier run in Foundry on #23's proofs, gas recorded beside refine's. |
| #41 Poseidon2 in the PVM | One Poseidon2 hash and a depth-40 insertion measured on PolkaJam against reference vectors, and how many insertions fit accumulate's 10M. |
| #42 recompiler R and f | R (gas per ns, recompiled) and f (slowdown against native) per JIP operation, measured on one x86-64 machine (CI), feeding the JIP's gas constants. |
| #44 UltraHonk's gain | The verifier's refine gas on PolkaJam split into MSM, scalar multiplication, pairing and the rest at 2^16 to 2^24, and the projected gas with the core calls, used by #34. |
| #45 end-to-end benchmark | Calls 101–104 timed from encoded bytes with every validity check, random and adversarial inputs, median, p99 and max, on x86 CI, feeding #34's constants in the GP's unit. |
| #21 the per-transaction kernel port | Closed as superseded by #28 once #23 and #26 hold, with a comment saying which of its parts survive. |
| #29 lessons | `docs/privacy/LESSONS.md` committed, each claim linked to evidence. |
| #30 architectures | `docs/privacy/ARCHITECTURES.md` committed with a recommendation; claims measured or marked as reasoning; at least the "proving inside JAM" candidate measured (fold or verify N proofs in refine). |
| #31 limits and the JIP decision | `docs/privacy/LIMITS.md` committed and the decision recorded on the issue. |
| #32 JIP draft | `docs/jip/FORMAT.md` (the JIP process, sourced) and a complete `docs/jip/jip-draft.md` in that format; skipped (closed with the reason) only if #31 decides no JIP. |
| #33 JIP reviews | `docs/jip/reviews/`: both personas' numbered comments, each's responses to the other's, and a synthesis table. |
| #34 JIP final | `docs/jip/jip-final.md` with a change log citing review comments, numbers reproducible from this repo, and a submission note for Aodh. |
| #28, #35 tracking | Stay open as `roadmap`; each carries a status comment listing what is done and what remains. |

## Phase 3 done

`<promise>PHASE 3 DONE</promise>` may be written only when every row above holds with its
evidence in the ledger and on its issue, the demo's checks still pass (`NET=pj6
nets/demo.sh up`, `check`, from clean), and CI is green on the last commit.

# Phase 4: Aztec on JAM, settling without the JIP

Phase 3 ended with `PHASE 3 DONE` (ledger 60). JIP-8 is kept in reserve
(`docs/jip/SUBMISSION.md`). Phase 4's goal, from Aodh (2026-10-24): complete Aztec on JAM
under today's specs. Every per-package cost was measured to fit without host calls: about
981M a checkpoint and 813M an epoch proof, against a tiny package's 1e9 (#23, #37, #39). The
settlement service becomes Aztec's Rollup, Inbox and Outbox, checked against Aztec v6's own
fixtures (aztec-packages v6.0.0, 0219f11, `l1-contracts/test/fixtures`). Aztec's own node then
settles on it, with Aztec's packages used as released.

The iteration steps and rules are Phase 3's, and the demo's checks must keep holding.

## The issues, and what closes each

| Issue | Closed when |
|---|---|
| #49 the checkpoint path | Its header hash and blobs hash equal all six of Aztec's fixtures', which carry no blob data (natively). On PolkaJam, checkpoints made by Aztec's released v6 packages, with their blobs, are accepted in order with Aztec's header hashes stored. It refuses a checkpoint out of order, a tampered header, a wrong blob and a wrong timestamp, and its gas fits a tiny package. |
| #50 the Inbox | Messages sent as deferred transfers on PolkaJam give the leaves and bucket rolling hashes Aztec's released code computes for them. (The fixtures' own hash can't be reproduced: they don't record the rollup version.) A checkpoint consuming them is accepted; skipped, regressed, mismatched or missing consumption is refused. |
| #51 the epoch path | A bb 6 UltraHonk proof in the epoch proof's shape, carrying the 113 public inputs Aztec's released code assembles for an epoch of its checkpoints, advances `tip:proven` on PolkaJam. A valid proof with one wrong public input is refused by accumulate, and the stored Outbox root equals the epoch's out hash. |
| #52 node integration decided | A recorded run of Aztec v6's local network lists every L1 method, contract and event the node uses. `docs/aztec/NODE.md` chooses the facade or the seams on that evidence. |
| #53 end to end | A PXE transaction is checkpointed by Aztec's unchanged sequencer onto the settlement service on PolkaJam, synced back by Aztec's archiver, and seen mined by the PXE. |
| #48 field-wise hooks | `crates/curve-hooks` holds them, the settlement service verifies with them, and its epoch path is re-measured on PolkaJam with every verdict as before. (The privacy service, which needs a re-soak, is #55.) |
| #54 signatures | The checkpoint path refuses a checkpoint without the slot proposer's valid signature over Aztec's payload digest, which is checked against Aztec's released code, and the gas is recorded on PolkaJam. |
| #28 tracking | A status comment at the phase's end. |

## Phase 4 done

`<promise>PHASE 4 DONE</promise>` may be written only when every row above holds with its
evidence in the ledger and on its issue, the demo's checks still pass from clean, and CI is
green on the last commit.
