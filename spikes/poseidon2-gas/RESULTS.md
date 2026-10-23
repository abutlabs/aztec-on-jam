# Poseidon2 in the PVM, on PolkaJam (#41)

`spikes/poseidon2-gas/measure.sh`, 2026-10-23, Apple M-series Mac, polkajam-testnet v0.8.0
(six validators, the PVM interpreter), via `spikes/lib/polkajam.sh`.

**Question.** Can a privacy service's accumulate (10M gas a report, GP 0.8.0) insert notes into
its own tree? If it could, proofs could name a recent root instead of the exact current one,
and many packages a slot could share one tree (#31, candidate 3).

**The code.** `p2/` is Poseidon2 over BN254 (t = 4, x^5, 8 full and 56 partial rounds; Noir's
permutation; noir-lang/poseidon's sponge) in no_std Rust on arkworks. It is a port of
`client/poseidon2.py`, with constants generated from the same JSON (`gen.py`). On the host it
matches Noir's 5 permutation vectors, `hash_cli`'s H(7,11,13) and H(0,0), and the empty
depth-16 root. The expected values below come from `client/poseidon2.py`, a second
implementation.

| case | refine gas | PolkaJam refine | verdict |
|---|---|---|---|
| 1 hash | 437,462 | 0.002 s | 01 |
| 10 hashes, chained | 4,297,147 | 0.007 s | 01 |
| 100 hashes, chained | 42,892,428 | 0.049 s | 01 |
| 1 depth-40 path (40 hashes) | 17,515,645 | 0.023 s | 01 |
| 10 depth-40 paths | 171,853,288 | 0.183 s | 01 |
| 1 path, wrong root expected | 17,515,643 | 0.022 s | 00 |

**Per hash: 428,839 gas** (the slope from 10 to 100). **Per depth-40 path: 17,148,627** (the
slope from 1 to 10). Gas is the PVM's, the same in accumulate as in refine.

**Answer: no.** Accumulate's 10M gas a report pays for 0.58 of one depth-40 path. A note
insertion needs one path, and an indexed-nullifier insertion about two (34M). So under
today's specs a privacy tree's updates must be computed in the proof (as this project's
circuit does) and checked against the exact current root, and one tree takes one package a
slot. A hash host call would change that; how much cheaper native Poseidon2 is will be measured
with the JIP's native timings (#32).
