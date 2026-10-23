# jam-privacy-service

Aztec's privacy runtime as a JAM service. Users prove private transfers on their own
device with Aztec's Noir kernel circuits; a builder folds the proofs; `refine` verifies
the folded proof and recomputes the note and nullifier trees from witnesses; `accumulate`
checks the old root, writes the new root and the nullifiers, and stores the encrypted
notes. Value enters and leaves by JAM deferred transfers: shield and unshield.

Why: [JAM beside Aztec](https://abutlabs.github.io/the-gray-tutor/jam-beside-aztec), the
comparison this grew out of. How: [docs/PLAN.md](docs/PLAN.md).

## The four repositories

| Repo | Role | Pinned at |
| --- | --- | --- |
| this one | the Rust service, the public-balance helper service, the nets | — |
| [abutlabs/aztec-packages](https://github.com/abutlabs/aztec-packages) | Noir protocol circuits (kernels, tx-base, merge) and `bb` | `v6.0.0` |
| [abutlabs/aztec-node](https://github.com/abutlabs/aztec-node) | TypeScript: the PXE, and the builder and indexer cut from the sequencer and archiver | `main` |
| [abutlabs/demo-wallet](https://github.com/abutlabs/demo-wallet) | the `zk.money` wallet: shield, send, unshield | `main` |

All four sit under `Aiden/submodules/`. Aztec's code is Apache-2.0; so is this.

## Layout

- `services/privacy/` — the service crate (`jam-pvm-common`, `no_std`), built with
  jamswap's `jam080-build` (GP 0.8.0) to `privacy-service.jam`.
- `spikes/ultrahonk-gas/` — the measurement that decides the architecture (plan, step 0).
- `nets/` — how the service lands on a local tiny net: jamswap's `lasair6` (six lasair
  validators), `pj6` (six PolkaJam validators) or the mixed net.
- `docs/PLAN.md` — the implementation plan, with its gates.

## Status

Scaffold. `jam080-build services/privacy --out <dir>` builds a 12 KB GP 0.8.0 blob whose refine rejects everything; step 0 of the plan is under way (#5).
