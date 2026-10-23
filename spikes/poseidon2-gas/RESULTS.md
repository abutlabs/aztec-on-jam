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

| case | refine gas | PolkaJam refine (3 guarantors, median) | verdict |
|---|---|---|---|
| 1 hash | 437,597 | 0.0017 s | 01 |
| 10 hashes, chained | 4,297,327 | 0.0069 s | 01 |
| 100 hashes, chained | 42,893,058 | 0.0482 s | 01 |
| 1 depth-40 path (40 hashes) | 17,515,995 | 0.0233 s | 01 |
| 10 depth-40 paths | 171,855,582 | 0.1832 s | 01 |
| 1 path, wrong root expected | 17,515,993 | 0.0211 s | 00 |

These are the current service's figures (with #43's kind 2). The first build, without it,
measured 2 gas less a hash.

**Per hash: 428,841 gas** (the slope from 10 to 100). **Per depth-40 path: 17,148,843** (the
slope from 1 to 10). Gas is the PVM's, the same in accumulate as in refine.

**Answer: no.** Accumulate's 10M gas a report pays for 0.58 of one depth-40 path. A note
insertion needs one path, and an indexed-nullifier insertion about two (34M). So under
today's specs a privacy tree's updates must be computed in the proof (as this project's
circuit does) and checked against the exact current root, and one tree takes one package a
slot. A hash host call would change that; how much cheaper native Poseidon2 is will be measured
with the JIP's native timings (#32).

## Appending a batch of notes (#43)

A note tree is append-only, so appending k notes in one refine shares the upper levels. Kind 2
appends k leaves to a depth-40 tree from its frontier (the 40 left nodes the next index pairs
with), level by level. Each level hashes its new nodes in pairs, so k notes cost about k + 40
hashes. The expected roots come from `tools/append.py`'s `sequential`, a different algorithm
(each leaf rehashes its whole path, as an incremental tree does). `tools/append.py` holds that
algorithm and the level-by-level one to a naive full tree, for every split into two appends at
depth 4. The tree starts at a random odd index past 2^39 with a random frontier.

| case | refine gas | PolkaJam refine (3 guarantors, median) | verdict |
|---|---|---|---|
| append 1 leaf | 17,917,418 | 0.0205 s | 01 |
| append 64 leaves | 46,380,829 | 0.0511 s | 01 |
| append 1,024 leaves | 466,117,396 | 0.4934 s | 01 |
| append 2,048 leaves | 915,618,413 | 0.9524 s | 01 |
| append 2,048 leaves, wrong root expected | 915,618,411 | 0.9521 s | 00 |

**Per appended note: 438,966 gas** (the slope from 1,024 to 2,048), 1.02 hashes. So one refine
appends **about 2,238 notes in a tiny package (1e9) and 11,350 in a full one (5e9)**, against
290 if each note rehashed a full path. The notes travel in the payload, 32 bytes each: above
about 6,300 they pass JAM-NP's 200 KiB message and go as extrinsics (#36, #37).

