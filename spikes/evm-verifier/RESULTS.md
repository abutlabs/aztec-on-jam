# Aztec's UltraHonk epoch verifier on Ethereum, measured (#40)

The Ethereum side of #23's comparison. bb 6.0.0 wrote the Solidity verifier for each of #23's
every-relation proofs (2^16, 2^20, 2^24; plain UltraHonk, Keccak transcript, 113 public
inputs), with `--optimized` as Aztec builds its epoch verifier
(`noir-protocol-circuits/bootstrap.sh:174`) and without. Foundry 1.7.1 (cancun, the optimizer
on, the EIP-170 size limit lifted for the test) verified the same proofs PolkaJam checked:
`forge test -vv` (`gen-tests.sh` writes one test per verifier). Each test asserts `true`, then
asserts a revert with one public input altered. All six pass.

| Verifier | 2^16 | 2^20 | 2^24 (Aztec's size) |
|---|---|---|---|
| `--optimized` (Aztec's) | 618,779 gas | 669,945 | **721,239** |
| plain | 1,632,783 | 1,847,625 | 2,063,360 |

The gas is the verifier's execution, measured with `gasleft()` around the call. A transaction
also pays calldata, about 16 gas a byte: 14,212 bytes for Aztec's proof and inputs, about
227,000 (Aztec's `gas_benchmark.md` reports 227,392).

**Beside JAM, like with like** (each as a share of the budget it runs in):
- JAM: 180,791,299 of a full package's 5e9 = 3.6% (#23).
- Ethereum: 721,239 of a 60M block = 1.2%.

**About 3 times the share**, against about 50 times for single checks that Ethereum prices as
precompiles (KZG point evaluation, ecrecover; `docs/privacy/LESSONS.md`). Ethereum's UltraHonk
verifier is contract code too. Only its curve operations use precompiles (0x06-0x08), so JAM
loses less ground on whole verifiers than on single precompiled checks.
