# Where an UltraHonk verification's gas goes, on PolkaJam (#44)

`spikes/verifier-profile/measure.sh`, 2026-10-23, Apple M-series Mac, polkajam-testnet v0.8.0
(six validators, the PVM interpreter), via `spikes/lib/polkajam.sh`.

**Question.** The simulated reviews (#33, GW-10 and ZW-1) asked how much of a verification
JIP-8's curve calls would replace. The verifier (abutlabs/ultrahonk_verifier `bb-v6`, 2ae44db)
sends every curve operation through zkVerify's `CurveHooks`. The default `()` hooks run upstream
arkworks after encoding and decoding each point (`ark-bn254-ext` v0.6.0, `base_impl`). `Gauge`
(service/src/lib.rs) runs the same arkworks code, but converts points field by field, and reads
the remaining gas (host call 0) around each hook. The proofs are #23's every-relation circuits:
plain UltraHonk with a Keccak transcript, as Aztec's epoch proof.

| circuit | hooks | refine gas | verdict | verify | MSM G1 (calls, points) | Miller loop (pairs) | final exp | rest of verify |
|---|---|---|---|---|---|---|---|---|
| 2^16 | () | 170,671,447 | 01 | 170,433,917 | | | | |
| 2^16 | Gauge | 147,768,403 | 01 | 147,531,398 | 66,812,755 (1, 54) | 29,621,937 (2) | 19,053,958 | 32,042,748 |
| 2^20 | () | 175,899,223 | 01 | 175,643,914 | | | | |
| 2^20 | Gauge | 151,850,737 | 01 | 151,595,428 | 68,990,468 (1, 58) | 29,636,133 (2) | 19,049,415 | 33,919,412 |
| 2^24 | () | 181,113,874 | 01 | 180,840,140 | | | | |
| 2^24 | Gauge | 155,924,836 | 01 | 155,651,102 | 71,158,380 (1, 62) | 29,621,556 (2) | 19,051,852 | 35,819,314 |

The verifier makes one G1 MSM (`number_of_unshifted + log_n + 2` points) and one 2-pair
pairing, with no other scalar multiplications and no G2 operation. "Verify" is the gas inside
`verify` alone, without parsing the payload. The default hooks' own gas can't be read without
changing them, so their rows show only totals.

**What it shows, at Aztec's 2^24:**
- **25.2M (14%) is the default hooks' encoding,** removable today in any service, with no JIP:
  181.1M against 155.9M.
- **The curve work is 119.8M,** 77% of the `Gauge` verification: the MSM 71.2M (46%), the Miller
  loop 29.6M and the final exponentiation 19.1M.
- **The rest is 35.8M,** 23%, which no call touches: sumcheck, Shplemini's scalar arithmetic,
  the transcript, and reading the proof. It grows by about 0.47M a doubling of the circuit. The
  reviewers' estimate assumed at most 9.8M here, the gas of a proof rejected at sumcheck, so it
  left out Shplemini's own field work.

**Projected with the core calls,** the MSM priced by interpolating the x86 native times
(633.2 µs at 16 points plus 20.34 µs a point) and the pairing at 1,279.5 µs for 2 pairs, times
R × m:

| pricing | MSM (62 points) | pairing (2 pairs) | rest | total | against `()` | against `Gauge` |
|---|---|---|---|---|---|---|
| the draft's, R = 8, m = 2 | 25,100,262 | 20,472,000 | 35,819,314 | 81,391,576 | 2.2× | 1.9× |
| the GP's sizing, R = 3.33, m = 2 | 10,463,672 | 8,534,265 | 35,819,314 | 54,817,251 | 3.3× | 2.8× |

These are upper bounds on the gain. The native times are bare kernels on prepared points, with
no decoding or subgroup checks, which #45 adds. A full package would check about 61 proofs at
R = 8, or 91 at the GP's sizing, against 27–32 today.
