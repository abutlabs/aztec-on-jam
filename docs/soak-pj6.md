# The hour on pj6, 2026-10-23

`client/soak.py --seconds 3600` on six PolkaJam validators (jamswap's pj6), from a clean net,
on the shipped service and circuit (depth-40 trees, the indexed nullifier tree, sealed notes)
with the client at 97f5c8e: one privacy package a slot, each a real proof of a zero-value
transfer chained on the last, the next sent once the last is Reported, all on core 0.

| measure | value |
|---|---|
| net | pj6 |
| seconds | 3613 |
| slot_s | 6.0 |
| core | 0 |
| submitted | 600 |
| landed | 600 |
| dropped | 0 |
| latency_s | {'p50': 13.43, 'p90': 14.98, 'p99': 15.78, 'max': 16.31} |
| prove_s_p50 | 0.744 |
| packages_per_minute | 9.96 |
| resends | 0 |
| prove_errors | 0 |
| starved_slots | 0 |
| pace_vs_one_a_slot | 1.0 |
| best | [9505409, 9506011] |
| finalized | [9505408, 9506010] |
| finality_longest_stall_s | 10.1 |
| verdict | PASS |

The net it left (1,200 notes, 1,200 nullifiers) then passed `nets/demo.sh check` twice (37
assertions each), `shield`, `send` and `unshield`, and the two-browser run
(`client/e2e/e2e.mjs`: 5 assertions, every proof made in its own browser).

## Earlier runs

- **76aac7c, 600 of 600, PASS (ledger 19). Void as evidence for the shipped code:** its client
  loaded before the indexed nullifier tree and depth 40 were committed, during the run.
- **1bbd96e, FAIL (ledger 28):** 137 submitted, 137 landed, none dropped, finality never
  stalled, but proofs slowed to a 22 s median and the run held 0.228 of a slot. The client
  rebuilt the indexed tree for every proof, rehashing the whole tree twice per insert: a
  quadratic cost, 96 s at 400 nullifiers. Fixed in 97f5c8e (client) and demo-wallet 8f6ded8
  (the wallet page, which had the same code).
