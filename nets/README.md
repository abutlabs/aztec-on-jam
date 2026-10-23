# Nets

Five six-validator JAM tiny nets run the demo. `NET=<name> nets/demo.sh up` starts one, deploys
both services and starts the indexer and the wallet; every later stage takes the same `NET`.

| `NET` | Validators | Launched by | JIP-2 endpoint | Clients |
| --- | --- | --- | --- | --- |
| `lasair6` (default) | 6 lasair | jamswap's Docker compose | lasair-reader | lasair 2.1.3 image |
| `pj6` | 6 PolkaJam | jamswap's Docker compose | an ordinary PolkaJam node | PolkaJam nightly-2026-09-22 |
| `jb-lasair6` | 6 lasair | our jamboree fork, native | lasair-reader | lasair main (`../lasair`, `--rig`) |
| `jb-pj6` | 6 PolkaJam | our jamboree fork, native | an ordinary PolkaJam node | PolkaJam v0.8.0 |
| `jb-mixed` | PolkaJam 0-2, lasair 3-5 | our jamboree fork, native | the PolkaJam node | PolkaJam v0.8.0 + lasair main |

On every net the demo talks to the chain the same way: `jamboree rpc` (deploy through the
Bootstrap service, submit work-packages, read storage, package status; `nets/ctl.sh`) and the
indexer's own JIP-2 client. On the Docker nets jamboree runs in a container on the net
(`scripts/jamboree-linux.sh` builds it), because their JIP-2 endpoint is not published to the
host. Nothing of jamswap's Python is used; jamswap provides the Docker nets and the GP 0.8.0
blob builder.

The two clients of a mixed net must agree on PVM gas to the unit: PolkaJam v0.8.0 and lasair
main do (the v0.8.0 vectors' cmov rule); the 2026-09-22 nightly does not, and a mixed net with
it forks at the first accumulate whose gas differs (ledger 23). Each Docker net's pair agrees.

`NET=jb-… nets/jb.sh headcheck [minutes]` runs jamboree's own check on a native net: the
validators alone, headless, JIP-3 telemetry to jamboree, all six on one finalized block.

| File | What |
| --- | --- |
| `demo.sh` | the stages: up, check, shield, send, unshield, wallet, down |
| `net.sh` | the Docker nets (jamswap's compose), `jamboree rpc` in a container |
| `jb.sh` | the native nets jamboree launches |
| `ctl.sh` | the three chain calls both use: head, read, submit |
