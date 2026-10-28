# Nets

Six-validator JAM tiny nets on this machine, no Docker. `services/aztec-settlement/devnet.sh`
brings one up for Aztec on JAM (through `tools/aztec-local/on-jam.sh`, with `DEVNET_NET`).

| `NET` | Validators | Launched by | JIP-2 endpoint | Clients |
| --- | --- | --- | --- | --- |
| `polkajam` (default) | 6 PolkaJam | PolkaJam's own `polkajam-testnet` | its ordinary node | PolkaJam v0.8.0 |
| `jb-lasair6` | 6 lasair | our jamboree fork | lasair validator 0 | lasair main (`../lasair`) |
| `jb-pj6` | 6 PolkaJam | our jamboree fork | an ordinary PolkaJam node | PolkaJam v0.8.0 |
| `jb-mixed` | PolkaJam 0-2, lasair 3-5 | our jamboree fork | the PolkaJam node | PolkaJam v0.8.0 + lasair main |

Every net runs its timeslots on the wall clock, as JAM does: PolkaJam's from JAM's Common Era,
an all-lasair net from its own start (`jb.sh` writes the era, lasair's `--era`), so a slot is
six seconds of real time however busy the machine is.

The `polkajam` net is PolkaJam's own launcher from the v0.8.0 release: six validators and one
ordinary node serving JIP-2, GRANDPA finality, nothing but the release's binaries. Its
validators use UDP 41000 to 41005 (`polkajam --chain dev:41000 dump-spec`); its JIP-2 endpoint
is 42950. The jb-* nets and the spikes' testnets (`spikes/lib/polkajam.sh`) use 40000 and
19800, so a spike can run beside one. `down` stops the launcher's nodes too, which the launcher
itself does not.

The two clients of a mixed net must agree on PVM gas to the unit: PolkaJam v0.8.0 and lasair
main do (the v0.8.0 vectors' cmov rule).

`NET=jb-… nets/jb.sh headcheck [minutes]` runs jamboree's own check: the validators alone,
headless, JIP-3 telemetry to jamboree, all six on one finalized block.
