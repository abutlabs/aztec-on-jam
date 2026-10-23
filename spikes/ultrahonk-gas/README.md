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

## Result, 2026-10-23: the build profile is most of the cost (#13)

The spike's 286M against the service's 167M for the same verifier was the profile: the
spike built with `opt-level = "z"` + LTO; the service, a workspace member, had its profile
ignored by Cargo and built at opt-level 3. Same proof (bb 5, zk), five profiles:

| opt-level | LTO | zk gas | plain gas | blob bytes |
|---|---|---|---|---|
| z | yes | 286,136,021 | 272,022,666 | 247,078 |
| s | yes | 238,812,374 | 226,956,797 | 317,789 |
| 2 | yes | 161,038,053 | 151,880,350 | 1,127,351 |
| 3 | yes | 159,389,725 | 150,351,717 | 1,157,121 |
| 3 | no | 162,842,117 | 153,808,685 | 1,183,381 |

Circuit size matters far less: at the same profile the transfer circuit (log n 14, 20 public
inputs with the 8 pairing limbs) costs 166.1M against this circuit's (log n 12, 9) 161.5M.
Both the services and this spike now build at opt-level 3, LTO, one codegen unit:

| bb | case | gas | output | proof bytes | steps |
|---|---|---|---|---|---|
| bb3 | plain | 171198181 | 01 | 6624 | 56095652 |
| bb3 | zk | 177314601 | 01 | 7488 | 58287034 |
| bb3 | zk, proof byte flipped (must be 00) | 6068625 | 00 | 7488 | 1587239 |
| bb3 | zk, wrong public input (must be 00) | 6058058 | 00 | 7488 | 1585194 |
| bb5 | plain | 152871367 | 01 | 6368 | 48981097 |
| bb5 | zk | 161500875 | 01 | 7232 | 52181298 |
| bb5 | zk, proof byte flipped (must be 00) | 331165 | 00 | 7232 | 84511 |
| bb5 | zk, wrong public input (must be 00) | 5193873 | 00 | 7232 | 1290236 |
