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
- **With call 105, about four proofs fit a package.** Each costs 0.98B to reduce, plus about 0.1B
  to fold its IPA claim into the batch (#46: 1.27 ms natively). The discharge becomes one call at
  its priced floor, about 0.65B (`docs/privacy/ARCHITECTURES.md`). (5.0 − 0.65) / 1.1 ≈ 4.
- **What remains is the reduction.** Step 1 (`spikes/chonk-ipa`) splits its native time: Poseidon2
  transcript hashing 43%, BN254 work 25% (calls 101 and 102), field arithmetic 32%.

## Not measured here

- The reduction with its BN254 work (KZG batch-mul, pairings) routed to calls 101 and 102. bb
  calls these inside its own templates, so routing them means changing bb or replacing its
  functions at link time. Step 1's split bounds the saving at about a quarter of the reduction.
- The batch discharge itself (`IPA::batch_verify_accumulators`) in refine. One proof's IPA verify
  stands in for it here, as natively the two cost the same (99 to 100 ms).
- Only one proof and one machine.
