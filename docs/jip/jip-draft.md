# JIP-8: Cryptographic host calls

Host calls for the elliptic-curve checks that zero-knowledge proofs and signatures need
(pairings and multi-scalar multiplication on BN254 and BLS12-381, EIP-4844 KZG point
evaluation, secp256k1 public-key recovery) and for Poseidon2, the hash their circuits use, so that a service verifies a proof or a
signature at close to native cost instead of as interpreted PVM code.

*README line:* `- [JIP-8](./JIP-8.md): **Cryptographic host calls** - Host calls for pairing checks, multi-scalar multiplication, KZG point evaluation and secp256k1 recovery, priced from native cost.`

*Status: first draft (2026-10-23), for review (#33). Not submitted. The number is the next
free one at polkadot-fellows/JIPs 5202db1 and may change.*

## Motivation

Privacy on JAM, as on any chain, rests on checking proofs and signatures: a user proves a
private transfer on their own device and the chain checks the proof. JAM is well placed for
this, because refine runs on cores with 5 billion gas a package and is re-executed by
auditors. But the PVM has no cryptographic host calls (GP 0.8.0, Appendix B: the host calls
are gas, fetch, lookup, read, write, info, export, machine, peek, poke, pages, invoke, expunge,
bless, assign, designate, checkpoint, new, upgrade, transfer, eject, query, solicit, forget,
yield, provide). Every pairing, curve multiplication and field inversion is interpreted PVM
code.

Measured on a six-validator PolkaJam v0.8.0 testnet, gas from PolkaJam's own telemetry
([jam-privacy-service](https://github.com/abutlabs/jam-privacy-service), each with its
script):

| Check | PVM gas in refine | Share of a full package (5e9) | Ethereum (precompile) | Share of a 60M block |
|---|---|---|---|---|
| UltraHonk proof (Noir/Barretenberg, Keccak transcript, 2^16–2^24 gates, every gate type in use; 2^24 is Aztec's epoch proof's size) | 170,364,998 – 180,791,299 | 3.4–3.6% | 618,779 – 721,239 (bb's optimized Solidity verifier, using the 0x06–0x08 precompiles) | 1.0–1.2% |
| EIP-4844 KZG point evaluation | 199,473,075 | 4.0% | 50,000 | 0.083% |
| One blob checked against its commitment | 262,430,040 | 5.2% | free to contracts (consensus) | |
| secp256k1 public-key recovery | 13,119,449 | 0.26% | 3,000 | 0.005% |
| Poseidon2 (BN254, t = 4), one hash | 428,839 | 0.009% (4.3% of accumulate's 10M) | none (circuits only) | |
| A depth-40 tree insertion (40 hashes) | 17.1M | 171% of accumulate's 10M | | |

Compared as shares of the budget each check runs in, a JAM service pays about 50 times what an
Ethereum contract pays for a precompiled check (KZG, ecrecover), and about 3 times for a whole
UltraHonk verifier, which on Ethereum is contract code using the curve precompiles. That price falls on every privacy system, rollup
settlement and cross-chain light client a JAM service could host. For example, settling the
Aztec network on JAM needs about 812M gas an epoch proof and 981M a checkpoint (proof, blobs,
attestations), most of it pairings and curve arithmetic (`docs/privacy/LESSONS.md`,
`docs/aztec/SERVICE.md`).

The checks themselves are standard and implemented in widely used, audited libraries (blst,
arkworks, k256), which every client already depends on or can. A host call runs them natively,
which is what this JIP proposes, and nothing more: no privacy-specific protocol features.

## Host-call specification

All calls take their input from memory, write their output to memory, and return a result
code in φ'7, following GP Appendix B's conventions:
- If the input range or output range is not accessible, the call **panics**, as other GP host
  calls do on inaccessible memory.
- If the input is malformed (a wrong length, a point not on the curve or not in the subgroup,
  a scalar not below the group order), φ'7 = `HUH` and no output is written.
- Otherwise φ'7 = `OK` and the output is written.

Encodings follow Ethereum's precompiles (EIP-196, EIP-197, EIP-2537, EIP-4844, `ecrecover`),
so their test vectors apply unchanged and services can reuse Ethereum's tooling.

Gas is charged in full **before** execution, from the input length alone, so the cost never
depends on the input's values. The constants `G_*` are set from benchmarks (see *Gas pricing*).

### 1. `bn254_g1_msm`

**Index**: 101

**Gas usage**: G_BN_MSM_BASE + n · G_BN_MSM_ITEM, with n = ⌊φ8 / 96⌋

**Input registers**: φ7 = input offset, φ8 = input length (a multiple of 96, n ≥ 1), φ9 = output offset

- input = μ_{φ7…+φ8}: n items of (G1 point: x, y as 32-byte big-endian; 0, 0 is the point at
  infinity), scalar 32-byte big-endian

**Output registers**: φ'7

- `OK`: μ'_{φ9…+64} = Σ scalar_i · point_i, as x, y
- `HUH`: φ8 is not a positive multiple of 96, a point is not on the curve, or a scalar is
  not below the group order

### 2. `bn254_pairing_check`

**Index**: 102

**Gas usage**: G_BN_PAIR_BASE + k · G_BN_PAIR_ITEM, with k = ⌊φ8 / 192⌋

**Input registers**: φ7 = input offset, φ8 = input length (a multiple of 192, k ≥ 1)

- input: k pairs of (G1 point, 64 bytes; G2 point, 128 bytes), as in EIP-197

**Output registers**: φ'7 = `OK` with φ'8 = 1 if ∏ e(G1_i, G2_i) = 1, else φ'8 = 0; `HUH` on a
malformed input (including a G2 point outside the subgroup)

### 3. `bls12_381_g1_msm`

**Index**: 103

**Gas usage**: G_BLS_MSM_BASE + n · G_BLS_MSM_ITEM, with n = ⌊φ8 / 160⌋

**Input registers**: as `bn254_g1_msm`, with EIP-2537's G1 encoding (128 bytes a point) and a
32-byte scalar, so 160 bytes an item

**Output registers**: `OK` with the 128-byte result at φ9, or `HUH`

### 4. `bls12_381_pairing_check`

**Index**: 104

**Gas usage**: G_BLS_PAIR_BASE + k · G_BLS_PAIR_ITEM, with k = ⌊φ8 / 384⌋

**Input registers**: as `bn254_pairing_check`, with EIP-2537's encodings (G1 128 bytes, G2 256
bytes)

**Output registers**: as `bn254_pairing_check`

### 5. `kzg_point_evaluation`

**Index**: 105

**Gas usage**: G_KZG

**Input registers**: φ7 = input offset (192 bytes: versioned hash, z, y, commitment, proof, as
EIP-4844's point-evaluation precompile)

**Output registers**: φ'7 = `OK` with φ'8 = 1 if the proof shows the commitment opens to y at z
and the versioned hash is 0x01 ‖ sha256(commitment)[1..], else φ'8 = 0; `HUH` on a malformed
input

The trusted setup is Ethereum's KZG ceremony output, the same [s]G2 as EIP-4844.

### 6. `secp256k1_recover`

**Index**: 106

**Gas usage**: G_ECRECOVER

**Input registers**: φ7 = input offset (128 bytes: hash, v, r, s, as Ethereum's 0x01 precompile),
φ8 = output offset

**Output registers**: φ'7 = `OK` with the signer's 20-byte address at φ8 (keccak256 of the public
key, last 20 bytes), or `HUH` if nothing recovers (v not 27 or 28, r or s not in [1, n)).
High s is accepted, as the precompile accepts it.

### 7. `poseidon2_bn254_permute`

**Index**: 107

**Gas usage**: G_POSEIDON2

**Input registers**: φ7 = state offset (4 BN254 field elements, 32-byte big-endian each, 128
bytes), φ8 = output offset

**Output registers**: φ'7 = `OK` with the permuted state at φ8 (128 bytes), or `HUH` if an
element is not below the field modulus

The permutation is Poseidon2 with t = 4, x^5, 8 full and 56 partial rounds, and the round
constants and internal diagonal Noir and Barretenberg use (pinned by their published vectors).
Sponges and tree hashing stay in the service. Motivation: a privacy tree's insertion in
accumulate. One Poseidon2 hash costs 428,839 gas as PVM code, and a depth-40 path 17.1M,
more than accumulate's 10M a report (jam-privacy-service#41). Without it, every tree update
must be proven and applied in order, one package a slot.

### Side-effects

None beyond the output memory. No state is read or written. Every call is available in
refine, accumulate and is-authorized.

## Gas pricing

A host call's gas should match what the same work would cost as PVM instructions on the GP's
reference hardware, so that a package's gas still bounds its time. We propose

    G = ⌈t_native × R × m⌉

where t_native is the call's single-threaded time with a reference library on the GP's
reference machine, R is the PVM's gas throughput there (gas per nanosecond for ordinary code
under a recompiler), and m ≥ 1 is a safety margin for worst-case inputs and slower clients.

The constants are to be set from a benchmark suite this JIP will carry. Every client runs it,
and the maximum across clients and hardware is taken. Today's measurements, for scale:
- PolkaJam's interpreter on Apple M-series ran 199.5M gas of pairing code in 0.19 s, about 1
  gas a nanosecond.
- Its recompiler on a 4-vCPU x86-64 runner ran 169M gas of UltraHonk verification in 8–24 ms
  in 15 of 30 runs (about 7–21 gas a nanosecond) and 0.41–0.56 s in 12, a variance whose cause
  is not yet known (jam-privacy-service#14).

Native timings for each call: to be measured (this draft, next step).

## Determinism and consensus

Unlike JIP-1's `log`, which returns `WHAT` whether or not a node implements it, these calls
change a service's result and gas. Every guarantor and auditor of a package must agree on
both, so every client must implement every call identically, from the same activation point.
**This JIP is therefore a protocol change: a GP amendment adopted at an activation slot, not
an optional extension.** Before activation the indices are unknown host calls, and return
`WHAT` as today.

Implementations must agree byte for byte on outputs and on `HUH`. That includes subgroup
checks, the canonical range of every field element and scalar, the point-at-infinity
encodings, and high-s handling in `secp256k1_recover`. The Ethereum precompile test vectors
pin these behaviours.

## Compatibility

New indices only. Existing services are unaffected, and a service may keep its PVM
implementation as a fallback before activation. Indices 101–107 follow JIP-1's 100, outside
the GP's 0–27.

## Test vectors

Ethereum's precompile vectors, unchanged in encoding:
- EIP-196 and EIP-197 (BN254);
- EIP-2537 (BLS12-381);
- c-kzg-4844's `verify_kzg_proof` (122 vectors, all agreeing with this project's
  implementation built natively; the same code ran in refine on PolkaJam);
- geth's `pointEvaluation` and `ecRecover`.

Plus vectors for the `HUH` cases above, and gas vectors: input lengths with their gas.

## Reference implementations

PVM implementations already exist, as services, and serve as the fallback and the benchmark
baseline:
- an UltraHonk verifier (zkVerify's, ported to bb 6.0.0);
- KZG on zkcrypto's `bls12_381`;
- `ecrecover` on `k256`.

Native implementations: blst (BLS12-381), arkworks (BN254), k256 (secp256k1), c-kzg-4844.

## Open questions

1. R and m: one reference throughput for all clients, or the slowest client's?
2. Poseidon2 is included (call 7) because accumulate cannot afford a tree insertion in PVM
   code (#41). Should other proof-system hashes join it (Poseidon over BN254 as in circom,
   Rescue)? This draft says no, until a measured case asks.
3. Should Grumpkin and the BN254 G2 MSM be included, for IPA-based (CHONK) and other verifiers?
4. A separate index per curve operation, or one call with a curve and operation selector?
