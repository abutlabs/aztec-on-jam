# The hour on pj6, 2026-10-23

`client/soak.py --seconds 3600` on six PolkaJam validators (jamswap's pj6), commit 76aac7c: one
privacy package a slot, each a real proof of a zero-value transfer chained on the last, the
next sent once the last is Reported, all on core 0.

| measure | value |
|---|---|
| net | pj6 |
| seconds | 3611 |
| slot_s | 6.0 |
| core | 0 |
| submitted | 600 |
| landed | 600 |
| dropped | 0 |
| latency_s | {'p50': 13.52, 'p90': 15.01, 'p99': 15.87, 'max': 16.15} |
| prove_s_p50 | 0.661 |
| packages_per_minute | 9.97 |
| resends | 0 |
| prove_errors | 0 |
| starved_slots | 0 |
| pace_vs_one_a_slot | 1.0 |
| best | [9500427, 9501028] |
| finalized | [9500426, 9501027] |
| finality_longest_stall_s | 10.1 |
| verdict | PASS |
