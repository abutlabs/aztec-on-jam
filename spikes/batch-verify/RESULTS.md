# Many users' proofs in one refine, on PolkaJam (#30)

`spikes/batch-verify/measure.sh`, 2026-10-23, Apple M-series Mac, polkajam-testnet v0.8.0 (six
validators, the PVM interpreter), via `spikes/lib/polkajam.sh`.

**Question.** ARCHITECTURES.md's candidate 3 has JAM check every user's proof directly, with no
folding prover. How many proofs does one refine check, and does the cost stay linear?

**The code.** `service/` verifies n UltraHonk proofs from one payload,
`[tag][n]{[vk_len u16][vk][pubs_n u16][pubs][proof_len u32][proof]}`, with zkVerify's verifier
as ported to bb 6.0.0 (abutlabs/ultrahonk_verifier `bb-v6`, 2ae44db). It stops at the first
proof that fails and reports 01 only if all pass. The proofs are #26's three (`spikes/settlement/
proofs` A, B, C: #23's 2^16 every-relation circuit, plain UltraHonk with Keccak, as Aztec's epoch
proof), cycled. Each item is 13,288 bytes with its key and inputs.

| proofs | payload | refine gas | per proof | PolkaJam refine | verdict |
|---|---|---|---|---|---|
| 1 | 13,290 B | 170,404,578 | 170,404,578 | 0.172 s | 01 |
| 4 | 53,154 B | 681,095,607 | 170,273,902 | 0.585 s | 01 |
| 8 | 106,306 B | 999,999,972, out of gas | | 0.840 s | none |
| 12 | 159,458 B | 999,999,966, out of gas | | 0.841 s | none |
| 4, the 4th's first public input altered | 53,154 B | 519,006,957 | | 0.452 s | 00 |

Gas is the same on all three guarantors. The dev chain's packages are tiny (1e9 gas), so 8
and 12 run out; at 170.3M a proof, a full package (5e9) checks 29. The tampered batch is three
proofs (510.8M) plus 8.2M to reject the fourth, close to #26's 8,330,413 for one altered proof.

**So** verification is linear, about 170.3M gas a proof, with no shared setup to amortize: 5 a
tiny package, 29 a full one. Above 15 a package the proofs pass JAM-NP's 200 KiB message (#36)
and must travel as extrinsics (#37). Across 341 cores that is about 9,900 proofs every 6 s.
