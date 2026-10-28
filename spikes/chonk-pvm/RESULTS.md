# Aztec's client-proof verifier, unchanged, in refine (#62 step 2)

`spikes/chonk-pvm/measure.sh`, 2026-10-27, this Mac (Apple M-series), lasair main (GP 0.8.0 gas
model). Barretenberg 6.0.0 (aztec-packages 0219f11), its C++ sources exported unchanged, compiled
for the PVM by `build.sh`. One Aztec client proof: `spikes/chonk`'s pinned sponsored-fee transfer.

Gas is the protocol's, the same on every conforming client. Times are this machine's only.

## It builds and runs

bb's own CHONK verifier (`ChonkNativeVerifier`) runs in refine and gives bb's own verdicts:
- the proof is accepted;
- the same proof with one octet flipped is rejected, at the Merge step as natively.

No RISC-V toolchain is installed. The pipeline is Apple clang → LLVM IR for rv64e → rustup's
`llc` → a static library linked into a Rust service by jamswap's `jam080-build`. bb's sources are
not changed. What the PVM lacks is supplied around them (`build.sh` says how):
- a libc++ and C sysroot: the macOS SDK's libc++ 21 headers, with picolibc's C headers;
- static constructors, which the PVM never runs: called from one generated function;
- `thread_local`, emulated;
- a small runtime for what the verifier reaches: `new`/`delete`, `std::string`, `shared_ptr`
  counts, guards, no-op mutexes. Everything else, on logging, exception and file paths, is a stub
  that logs its name and stops.

bb's log level is set to silent, as bb's embedders do. A refine has nowhere to print.

## Code size

| Build | Service code | Under the 4 MB limit (W_C)? |
|---|---|---|
| `-O3`, bb's forced inlining | 5.67 MB | no |
| `-O3`, inlining left to LLVM (`plain-inline.h`) | 1.47 MB | yes |
| `-Os`, inlining left to LLVM | 1.30 MB | yes |

bb forces its field arithmetic inline (`BB_INLINE`). On rv64e, with 16 registers, that makes the
ECCVM and Translator relations about 2 MB alone. Natively, on arm64, the same verifier is 2.27 MB.
Leaving inlining to LLVM, by redefining `BB_INLINE` from outside bb, fits the limit and costs
almost nothing (below). `-O3` with plain inlining is the build used from here on.

## Gas

| Payload | Native (one thread) | Refine gas, `-O3` plain | Refine gas, `-O3` forced | Refine gas, `-Os` plain |
|---|---|---|---|---|
| The per-proof reduction | 18.4 ms | **980.4M** | 991.1M | 1,017.9M |
| The same, one proof octet flipped (rejected) | 6.6 ms | 379.0M | 350.0M | 398.1M |
| The reduction, with the 2^15 Grumpkin CRS points parsed | 20.9 ms | 1,241.7M | 1,257.0M | 1,376.1M |
| The whole verification: reduction and the proof's own 2^15 IPA | 125.0 ms | **11.03B** | 10.26B | 11.83B |

From these:
- **A proof's reduction costs 0.98B gas**: Merge, ECCVM, Translator, MegaZK and their pairings,
  everything but the IPA.
- **Parsing the 2^15 Grumpkin points costs 261M** (about 8,000 gas a point). A service would hold
  them in its preimages, already in the form bb uses.
- **One 2^15 IPA (the Grumpkin MSM) costs 9.0 to 9.8B**, more than a whole package (5e9).
- **Memory fits easily.** jam-pvm-common's bump allocator never frees, yet the reduction ends
  below 1.4 MiB of address space and the whole verification below 40 MiB.

### Against #46's estimate

#46 scaled native times by one ratio, 70 to 77M gas a native millisecond, taken from our
UltraHonk verifier. Measured, the two halves of a CHONK check differ:

| | Estimated (#46) | Measured | Gas a native ms |
|---|---|---|---|
| Per-proof reduction | 1.4 to 1.6B | 0.98B | 53M |
| One 2^15 IPA (the discharge's MSM) | 7.0 to 7.7B | 9.0 to 9.8B | 87 to 94M |

**The reduction is 35% cheaper than estimated, and the MSM 25% dearer.** Curve arithmetic over
Grumpkin's base field costs the PVM more per native millisecond than the sumcheck's field work.

## What it means for checking Aztec's proofs in refine

- **Without call 105, the check does not fit.** A batch's IPA discharge is one 2^15 MSM, about
  100 ms natively (`spikes/chonk-ipa`), so about 9 to 10B gas: two full packages. A lone proof's
  whole check is 11.0B. The MSM can be split, since its parts add up, but then each batch needs
  two or three packages for the discharge, plus an accumulate that adds their results.
- **With call 105, two proofs fit a package that discharges its own batch.** Each costs 0.98B to
  reduce, plus about 0.1B to fold its IPA claim into the batch (#46: 1.27 ms natively). The
  discharge becomes one call, G_GK_MSM(32,768) = 1.93B (priced in lasair#130):
  2 × 1.08 + 1.93 = 4.1B. Packages that only reduce (four proofs, 4.3B) could instead hand their
  claims to one discharge package, about 30 claims a call: about four proofs a package overall
  (not built). This page first said four a package with its own discharge, from an estimated
  call price of 0.65B; the priced call is 1.93B.
- **What remains is the reduction.** Step 1 (`spikes/chonk-ipa`) splits its native time: Poseidon2
  transcript hashing 43%, BN254 work 25% (calls 101 and 102), field arithmetic 32%.

## With call 105, on a live lasair net (2026-10-27)

`gk105.cpp` routes bb's Grumpkin MSM to the proposed host call 105. The service links with
`--wrap` for bb's two Grumpkin MSM entry points, so bb is not changed; without the call, bb's own
MSM runs. The same proof, in lasair's refine:

| Payload | Refine gas, without 105 | With 105 | bb's MSMs done by 105 |
|---|---|---|---|
| The whole verification (2^15 CRS points parsed) | 11.36B | **3.47B** | 7 |
| The per-proof reduction | 982M | 814M | 4 |
| One proof octet flipped (rejected) | 379M | 379M | 0 |

**One client proof's whole check fits a full-spec package (5e9).** A tiny-spec package (1e9)
holds only the reduction.

`on-lasair.sh` runs it on `nets/jb.sh`'s jb-lasair6: six lasair validators (lasair main), calls
101–105 active on every node, and G_R = 5e9 in the chain spec's protocol parameters, the full
spec's refine gas on an otherwise tiny chain (tiny's 1e9 is less than one check). Each proof is
one work-item. Its payload is [mode][tag], and the verifier's 2.18 MB input travels as the item's
extrinsic, because a package itself is held to 200 KiB.

| Work-item | Verdict, accumulated | Verifier gas | Finalized | Submit to final |
|---|---|---|---|---|
| Proof 1 | VALID | 3,456,104,321 | slot 14 | 21 s |
| Proof 2 | VALID | 3,456,104,321 | slot 18 | 26 s |
| Proof 3 | VALID | 3,456,104,321 | slot 22 | 24 s |
| Proof 4, one octet flipped | INVALID | 625,503,052 | slot 25 | 18 s |

- Every report was guaranteed with three signatures and audited by more than two-thirds of the
  six validators. The audits re-executed the refine from the bundle fetched over CE-147.
- lasair's refine took 7.8 to 13.3 s on this Mac, with six nodes sharing it.
- On the tiny chain (G_R = 1e9), the reduction alone also finalized: VALID, and INVALID for the
  flipped proof. The whole check ran out of gas there.
- 2026-10-27, later: a **second, distinct Aztec client proof** — the 7-layer storage proof, the
  heaviest pinned flow (10 circuits, 128,192 bytes) — verified on the same kind of net: VALID,
  verifier gas 3,729,308,707, 7 MSMs by call 105, finalized at slot 110. Each flow carries its
  own hiding VK (public-input counts differ): the transfer's is `out/chonk/vk`, the storage
  flow's is `storage-hiding.vk` here; the AMM flow's VK differs too and was not kept, so the
  AMM payload is refused by Oink's public-input check (312M gas, no verdict) — the right VK
  travels with the extrinsic, nothing is pinned on chain.

## The CRS as the service's preimage (#73, 2026-10-28)

The 2^15 Grumpkin points (with the BN254 point and G2) are the service's own preimage, in bb's
in-memory form (`crs_raw.cpp`, written by bb itself: each point read with bb's `from_buffer`,
its bytes copied out; 2,097,352 octets, blake2b `d4d4d86a…`). Its hash is fixed in the service's
code, so no other CRS can stand in. One work item (mode 0x40) has accumulate solicit it, `jamboree
rpc provide` hands it to the chain once, and refine reads it with `historical_lookup` and copies
the points instead of parsing them (`chonk_pvm_verify_crs` in `shim.cpp`). Each proof's
extrinsic is its proof and vk alone. Natively the verdicts are the same either way (`native_shim`
with and without `crs.raw`; the flipped proof is rejected).

On six lasair validators (`on-lasair.sh`, finalized each):

| Work-item | Verdict | Verifier gas | Before (CRS in the extrinsic) | Extrinsic |
|---|---|---|---|---|
| Transfer proof, ×2 | VALID | 3,263,329,969 | 3,456,104,321 | 84 KB (was 2.18 MB) |
| One proof octet flipped | INVALID | 432,728,272 | 625,503,052 | 84 KB |
| The 7-layer storage proof | VALID | 3,536,573,652 | 3,729,308,707 | 132 KB |

**193M gas a proof saved** (the points' parsing), and 2 MB less data per proof: a package of
two proofs now carries 0.2 MB of extrinsics, not 4.4 MB.

Found on the way, a lasair bug: refine's `historical_lookup` found nothing but a service's code.
A guarantor and an auditor ran refine over an empty service account; the lookup now reads the
state at the package's lookup anchor (lasair 690e223, with a test).

## Batches: several proofs, one IPA (#73, 2026-10-28)

`chonk_pvm_verify_batch` (`shim.cpp`) checks proofs the way bb's `ChonkBatchVerifier` does,
without its threads: each proof reduced to its deferred TripleIPA opening, then every passing
proof's accumulator checked together by `ECCVMVerifier::batch_verify_accumulators`, whose one
2^15 Grumpkin MSM goes to call 105. Mode 0x82; a verdict a proof: 1 valid, 0 its reduction
failed, 4 the batch IPA failed. Natively, the verdicts are bb's (two valid; valid and tampered;
three; and 4 for every proof with one CRS point corrupted, the branch no tampered proof reaches:
the IPA proof is bound into the transcript, so its tampering fails the reduction first).

On six lasair validators, G_R = 5e9 a package:

| Work-item | Verdicts | Verifier gas | A proof |
|---|---|---|---|
| Two transfer proofs | VALID VALID | 4,207,258,083 | 2.10B (3.26B alone) |
| A transfer and a tampered proof | VALID INVALID | 3,632,008,649 | — |
| Three (transfer, storage, transfer) | out of gas at 5e9 | — | — |
| Four | out of gas at 5e9 | — | — |

**Two proofs fit a package that discharges its own batch**: the second proof adds about 0.95B
to one proof's 3.26B. A third does not fit: reductions are 0.8 to 1.1B each, and the discharge's
call is 1.93B. More a package would mean packages that only reduce, handing their claims to one
that discharges (not built).

A proof that does not deserialize stops the refine: bb throws, and the PVM has no unwinding, so
the whole batch gets no verdict. A batch is therefore built from proofs already checked
natively, as Aztec's sequencer checks each proof on arrival; a malformed proof costs only its
own package.

## Not measured here

- The reduction with its BN254 work (KZG batch-mul, pairings) routed to calls 101 and 102. bb
  calls these inside its own templates, so routing them means changing bb or replacing its
  functions at link time. Step 1's split bounds the saving at about a quarter of the reduction.
- The batch discharge itself (`IPA::batch_verify_accumulators`) in refine. One proof's IPA verify
  stands in for it here, as natively the two cost the same (99 to 100 ms).
- Only one proof and one machine.
