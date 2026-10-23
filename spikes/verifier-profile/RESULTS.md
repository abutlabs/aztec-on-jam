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
| 2^16 | () | 170,681,509 | 01 | 170,444,791 | | | | |
| 2^16 | Gauge | 147,882,340 | 01 | 147,646,147 | 66,795,773 (1, 54) | 29,621,913 (2) | 19,054,251 | 32,174,210 |
| 2^20 | () | 175,911,957 | 01 | 175,657,460 | | | | |
| 2^20 | Gauge | 151,965,168 | 01 | 151,710,671 | 68,970,944 (1, 58) | 29,636,109 (2) | 19,049,626 | 34,053,992 |
| 2^24 | () | 181,129,375 | 01 | 180,856,453 | | | | |
| 2^24 | Gauge | 156,038,093 | 01 | 155,765,171 | 71,135,018 (1, 62) | 29,621,532 (2) | 19,052,106 | 35,956,515 |

The verifier makes one G1 MSM (`number_of_unshifted + log_n + 2` points) and one 2-pair
pairing, with no other scalar multiplications and no G2 operation. "Verify" is the gas inside
`verify` alone, without parsing the payload. The default hooks' own gas can't be read without
changing them, so their rows show only totals.

**What it shows, at Aztec's 2^24:**
- **25.1M (14%) is the default hooks' encoding,** removable today in any service, with no JIP:
  181.1M against 156.0M (#48).
- **The curve work is 119.8M,** 77% of the `Gauge` verification: the MSM 71.1M (46%), the Miller
  loop 29.6M and the final exponentiation 19.1M.
- **The rest is 36.0M,** 23%, which no call touches: sumcheck, Shplemini's scalar arithmetic,
  the transcript, and reading the proof. It grows by about 0.47M a doubling of the circuit. The
  reviewers' estimate assumed at most 9.8M here, the gas of a proof rejected at sumcheck, so it
  left out Shplemini's own field work.

## The calls as PVM code (#45)

Payload kind 2 runs `bn254_g1_msm` and `bn254_pairing_check` as PVM code, end to end. These are
the same functions as `spikes/jip-bench`'s e2e: decoding, every check, the result. The inputs
are random group elements, made outside the metered region. Beside each is the gas the final
JIP would charge as a host call, from #45's end-to-end x86 timings (`spikes/jip-bench/RESULTS.md`):

| call | n | as PVM code (metered) | as the call | saving |
|---|---|---|---|---|
| bn254_g1_msm | 1 | 9,378,971 | 703,667 | 13.3× |
| bn254_g1_msm | 16 | 26,223,677 | 3,178,140 | 8.3× |
| bn254_g1_msm | 64 | 75,204,219 | 8,007,640 | 9.4× |
| bn254_g1_msm | 128 | 110,727,638 | 12,852,234 | 8.6× |
| bn254_pairing_check | 1 | 44,760,145 | 6,227,547 | 7.2× |
| bn254_pairing_check | 2 | 64,337,004 | 8,258,199 | 7.8× |
| bn254_pairing_check | 4 | 103,504,140 | 12,319,503 | 8.4× |
| bn254_pairing_check | 8 | 187,897,550 | 20,442,111 | 9.2× |

A one-point call (703,667) costs far less than the same scalar multiplication as PVM code
(9,378,971), which answers ZW-3's question.

**Projected with the core calls,** the MSM priced from #45's table (interpolated between 32
and 64 points) and the pairing at 4,196,895 + 2 × 2,030,652:

| circuit | MSM (points) | pairing (2 pairs) | rest | total | against `()` | against `Gauge` | proofs a full package |
|---|---|---|---|---|---|---|---|
| 2^16 | 7,508,992 (54) | 8,258,199 | 32,174,210 | 47,941,401 | 3.6× | 3.1× | 104 |
| 2^20 | 7,708,451 (58) | 8,258,199 | 34,053,992 | 50,020,642 | 3.5× | 3.0× | 99 |
| 2^24 | 7,907,910 (62) | 8,258,199 | 35,956,515 | 52,122,624 | 3.5× | 3.0× | 95 |

So JIP-8 would make an Aztec-sized UltraHonk check about 3.5× cheaper, 95 to a full package
against 27 today. The two curve calls fall to about 16M, and the 36M the calls don't touch is
now 69% of what remains. The first projection, at the draft's R = 8 and bare-kernel timings,
gave 2.2×. The end-to-end timings cost more, and the GP-anchored invariant prices them lower.

