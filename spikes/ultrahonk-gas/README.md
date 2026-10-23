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

## Result, 2026-10-23: bb 3.0.3 against bb 5.0.0

`./measure.sh` proves the same circuit with each toolchain (Noir beta.16 + bb 3.0.3; Noir
rc.5 + bb 5.0.0, `write_vk -t evm` then `prove -k`) and meters the matching verifier
(zkVerify main 986b79b; zkVerify PR #9 at a4b2a4af5d, in abutlabs/ultrahonk_verifier `bb-v5`):

| bb | case | gas | output | proof bytes | steps |
|---|---|---|---|---|---|
| bb3 | plain | 302,381,955 | 01 | 6,624 | 78,881,428 |
| bb3 | zk | 312,260,095 | 01 | 7,488 | 81,671,170 |
| bb3 | zk, proof byte flipped | 12,802,922 | 00 | 7,488 | 2,549,269 |
| bb3 | zk, wrong public input | 12,798,233 | 00 | 7,488 | 2,548,477 |
| bb5 | plain | 272,022,666 | 01 | 6,368 | 69,805,293 |
| bb5 | zk | 286,136,021 | 01 | 7,232 | 73,925,138 |
| bb5 | zk, proof byte flipped | 509,110 | 00 | 7,232 | 102,523 |
| bb5 | zk, wrong public input | 9,748,240 | 00 | 7,232 | 1,905,652 |

What bb 5 changed in the proof (from the two generated Solidity verifiers): the recursion
pairing-point object is 8 limbs (136 + 120 bits per coordinate) instead of 16 (4 × 68),
the 256 bytes; the ZK masking commitment moves to just after it; the padded proof constant
drops from 28 to 25. A flipped byte early in a bb 5 proof now fails the limb range check at
once (509 thousand gas).
