# Spike: UltraHonk verifier gas on the PVM

The gate in `docs/PLAN.md`, step 0. Run 2026-10-22; results in issue #13 and below.

1. Take a `no_std` Rust UltraHonk verifier (`ultrahonk_soroban_verifier`, BN254, Keccak
   transcript; pin the Noir and Barretenberg versions it targets).
2. Make one proof with `bb` of a small Noir circuit (the Noir `hello_world` is enough),
   with the Keccak oracle, and export its verification key.
3. Wrap verifier, key and proof in a refine the way `zk-jam-service/spikes/groth16-gas`
   does; build with `cargo jam-build`; run under the gas meter; confirm polkavm agrees.
4. Record: gas, blob size, stack needed, wall time on lasair. Write the decision row.

Comparison point: Groth16/BN254 verify, 246,884,255 gas under GP 0.8.0 (lasair,
2026-09-24, polkavm identical).

## Result, 2026-10-22

`./measure.sh` (Noir 1.0.0-beta.16, bb 3.0.3, zkVerify ultrahonk-no-std 986b79b, jam080-build, lasair demo_deploy):

| case | gas | output |
|---|---|---|
| plain | 302,381,955 | 01 |
| zk | 312,279,376 | 01 |
| zk, proof byte flipped | 12,801,522 | 00 |
| zk, wrong public input | 12,796,090 | 00 |

Blob 263,803 bytes; stack 8 MiB; about 0.9 s wall on lasair including deploy and accumulate.
Proving: `bb prove -s ultra_honk --oracle_hash keccak [--disable_zk] -b target/circuit.json -w target/circuit.gz -o target/<zk|plain> --write_vk`.
