# Ledger

Appended by every iteration of the build loop (`docs/RALPH.md`). Newest at the bottom.

## 2026-10-22 0 — scaffold
Did: repositories created and forked; plan written; service crate's entry points and item
layout; the scaffold builds to a 16 KB blob with `jam-pvm-build 0.1.28`.
Evidence: `jam-pvm-build -m service` → `Written JAM-PVM blob for privacy-service … 15846 bytes`
Next: step 0 of the plan, the verifier measurement, in `spikes/ultrahonk-gas/`. Install
`aztec-up` for `bb` and `aztec-nargo` first; record versions in `docs/TOOLS.md`.
Blockers: none known. Docker Desktop is installed (/Applications/Docker.app) but its daemon was not running at 2026-10-22 22:50; `open -a Docker` and wait for `docker info` before any compose step.
