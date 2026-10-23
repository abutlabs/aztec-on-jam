# Ethereum's ecrecover in refine, on PolkaJam (#39)

`spikes/ecdsa-gas/measure.sh`, 2026-10-23, Apple M-series Mac, polkajam-testnet v0.8.0 (six
validators, the PVM interpreter), via `spikes/lib/polkajam.sh`.

**Why.** Aztec's Rollup checks a proposer's ECDSA signature (secp256k1, an EIP-712 digest) on
every checkpoint (`ProposeLib.sol:259`), and the committee's attestations, 33 of 48, on an
epoch proof's last checkpoint (`EpochProofLib.sol`, v6.0.0). Ethereum does this with the 0x01
`ecrecover` precompile at 3,000 gas each. On JAM it is PVM code.

**The code.** `ecrecover/` is the precompile's semantics on k256 0.13 (no_std): v of 27 or 28,
r and s below n, and the address as the last 20 bytes of keccak256 of the public key. High s is
accepted, as the precompile does (only transactions require low s): k256 recovers from low s,
so a high s is negated and the recovery id's y-parity flipped. geth's own valid vector has a
high s, and failed until this was handled.

**Checked first, on the host:** all 5 of geth's `ecRecover.json` precompile vectors agree (1
valid, 4 that return nothing). 40 signatures made by Python's `cryptography` (40 keys, low s)
each recover their own signer's address.

| case | refine gas | of a tiny package (1e9) | of a full one (5e9) | PolkaJam refine (3 guarantors) | accumulate's verdict |
|---|---|---|---|---|---|
| 1 signature (geth ValidKey, high s) | 13,119,449 | 1.3% | 0.3% | 0.023 s | 01 |
| 33 signatures (an epoch's attestations) | 432,756,815 | 43.3% | 8.7% | 0.522 s | 01 |
| 33, the 17th signer swapped | 222,947,408 (stops at the 17th) | 22.3% | 4.5% | 0.289 s | 00 |
| geth InvalidHighV | 6,379 | 0.0% | 0.0% | 0.000 s | 00 |

About 13.1M gas a signature. **Per package, with #23 and #24:**
- An epoch proof is about 180M for the proof, 199M for the blob point evaluation and 433M for
  33 attestations: about 812M, so it fits a tiny package.
- A checkpoint is 968M for its 6 blobs batched plus 13M for the proposer's signature: about
  981M, which also fits.
