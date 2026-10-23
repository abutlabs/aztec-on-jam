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
