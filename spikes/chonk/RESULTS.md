# Aztec's own kernels, measured (#18)

`spikes/chonk/measure.sh`, 2026-10-23, on an Apple M-series Mac (10 cores). Aztec v6.0.0's
toolchain: Noir 1.0.0-rc.3 (the commit Aztec pins), Barretenberg 6.0.0. The inputs are Aztec
CI's pinned benchmark stacks (`chonk-inputs.hash` afe8389cb7f21295): for each transaction, the
app circuits and the private kernels, with their witnesses.

## Proving: the sender's side

| Flow | Circuits folded | Prove (10 threads) | Proof |
| --- | --- | --- | --- |
| a token transfer, fee sponsored | 6 | 2.2 s | 80,096 bytes |
| a token transfer, fee paid privately | 10 | 3.2 s | 128,192 bytes |
| an AMM add-liquidity | 12 | 4.0 s | 128,192 bytes |
| a 7-layer storage proof | 10 | 8.2 s | 128,192 bytes |
| all 11 flows | 6 to 12 | 2.2 to 8.2 s | 80 or 128 KB |

Peak memory for the transfer: 544 MB. Our join-split proves in 0.66 s natively, 1.0 s in a
browser tab, into 8,768 bytes.

## Verifying: the chain's side

One thread, as the PVM has (CPU time per verification, startup subtracted, three runs):

| Proof | Native verification |
| --- | --- |
| ours, UltraHonk, 8,768 bytes | 2.2 to 2.4 ms |
| Aztec's CHONK, 80,096 bytes | 164 to 173 ms |
| Aztec's CHONK, 128,192 bytes | 173 ms |

CHONK verification costs 70 to 79 times ours. Our verifier costs 169,112,761 gas in refine, so
by that ratio a CHONK verifier would need **about 12 billion gas** (11.8 to 13.4): 2.4 full
packages (5 billion each) or 12 tiny ones. The ratio is an estimate: CHONK's verifier spends
its time in an IPA check over Grumpkin, linear in circuit size, where ours spends it in BN254
pairings, and the PVM may price the two differently. No PVM (Rust, no_std) CHONK verifier exists
to measure directly; Barretenberg's is C++.

## Compiling: our toolchain on Aztec's source

| Circuit (v6.0.0 source, nargo 1.0.0-rc.3) | Ours | Aztec's benchmark stack |
| --- | --- | --- |
| `private_kernel_init_3` | 29,146 opcodes, 78,019 gates | 29,131, 78,007 |
| `private_kernel_reset_tail` | 89,717 opcodes, 312,773 gates | 31,997, 69,245 (a sized variant) |
| `hiding_kernel_to_rollup` | 1,420 opcodes, 40,502 gates | identical bytecode |

The kernels compile with Aztec's pinned Noir from a sparse checkout (21 MB) and the workspace
Aztec's own script generates. The init kernel differs by 15 opcodes from Aztec's pinned build
artifacts. The reset kernel's base crate is the largest of the variants Aztec generates; a
transaction uses the smallest that fits.

## What a kernel-based port would take

- **The proof does not fit refine.** About 12 billion gas against a full package's 5 billion.
  The builder would recursively verify each CHONK proof and wrap it into an UltraHonk proof
  refine can check, which is what Aztec's transaction-base rollup circuit and its provers do.
  The builder becomes Aztec's prover, and a transaction waits on that proof as well as the
  sender's.
- **The state is Aztec's.** The kernels read and write Aztec's trees: note hashes, nullifiers,
  public data, the archive, with Aztec's hashing and sizes. accumulate would keep those trees,
  a port of Aztec's world state to Rust.
- **The wallet is Aztec's PXE.** It proves the kernel stack (2 to 8 s, 0.5 GB) and talks to an
  Aztec node's API; repointed at an indexer over JIP-2.
- **The contracts carry over unchanged.** Aztec.nr contracts and their app circuits are what the
  kernels verify.

That is more than this loop. It is one `roadmap` issue; the demo keeps its own join-split, built
to Aztec's shapes (notes, an indexed nullifier tree, depth-40 trees, sealed notes).
