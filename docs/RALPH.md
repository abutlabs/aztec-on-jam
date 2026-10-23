# The build loop

This is the brief a Ralph loop reads at the start of every iteration. The same text each
time; the state is in the files and the git history. Read, in this order:

1. `docs/LEDGER.md` — what earlier iterations did, with evidence, and what is next.
2. `docs/PLAN.md` — the steps and gates. The ledger overrides the plan where they differ.
3. The code.

Then do the next step the ledger names. One step an iteration, verified, committed,
pushed, ledgered. Don't start a second step in the same iteration.

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
