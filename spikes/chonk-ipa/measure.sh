#!/usr/bin/env bash
# #46: a CHONK proof's verification split into the per-proof reduction and the deferred IPA, with
# Barretenberg 6.0.0 on one thread, and the IPA's discharge timed for batches of 1, 8, 64 and 512.
#   1. bb 6.0.0's own CHONK verify of one pinned flow (spikes/chonk's proof), CPU time per
#      verification with bb's startup subtracted: the whole check.
#   2. Barretenberg 6.0.0 built from source with MULTITHREADING=OFF, its ipa_bench patched
#      (ipa_bench.patch: ipa_verify fixed to export its proof once, and ipa_batch_discharge added):
#      IPA verify at 2^15 (CONST_ECCVM_LOG_N), and IPA<Grumpkin>::batch_verify_accumulators over
#      N deferred accumulators. The discharge's cost does not depend on the accumulators' values.
#   3. bb's own timing scopes (BB_BENCH) extended to verify and its steps (bb_verify_bench.patch):
#      the CRS setup, each step, Poseidon2 transcript hashing, BN254 KZG batch-mul and pairings.
# Needs: spikes/chonk/measure.sh run first (out/chonk), bb 6.0.0 at ~/.bb/versions/bb-6.0.0, and an
# aztec-packages v6.0.0 checkout (BB_SRC, default ../bb-v6.0.0) with cmake, ninja and clang.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd); HERE=$ROOT/spikes/chonk-ipa
BB6=$HOME/.bb/versions/bb-6.0.0
SRC=${BB_SRC:-$ROOT/../bb-v6.0.0}/barretenberg/cpp
F=ecdsar1+transfer_0_recursions+sponsored_fpc
export HARDWARE_CONCURRENCY=1

echo "== 1. bb 6.0.0 CHONK verify, one thread ($F)"
cpu() { /usr/bin/time -p bash -c "for i in \$(seq 1 $1); do $2 >/dev/null 2>&1; done" 2>&1 | awk '/^user/{u=$2}/^sys/{s=$2}END{print u+s}'; }
for r in 1 2 3; do
  ch=$(cpu 20 "$BB6 verify --scheme chonk -p '$ROOT/out/chonk/flows/$F/proof' -k $ROOT/out/chonk/vk")
  c0=$(cpu 20 "$BB6 --version")
  python3 -c "print('  %.1f ms a verification' % ((float('$ch') - float('$c0')) / 20 * 1000))"
done

echo "== 2. the IPA, from source (MULTITHREADING=OFF)"
( cd "$SRC"
  git apply --check "$HERE/ipa_bench.patch" 2>/dev/null && git apply "$HERE/ipa_bench.patch" || true
  [ -f build/build.ninja ] || cmake --preset default -DMULTITHREADING=OFF >/dev/null
  ninja -C build ipa_bench >/dev/null
  for r in 1 2; do ./build/bin/ipa_bench 2>/dev/null | grep -E '^ipa_(verify|batch)'; done )

echo "== 3. bb's own timing scopes on verify (bb_verify_bench.patch; threading on, run on one thread)"
( cd "$SRC"
  git apply --check "$HERE/bb_verify_bench.patch" 2>/dev/null && git apply "$HERE/bb_verify_bench.patch" || true
  [ -f build-mt/build.ninja ] || cmake --preset default -B build-mt >/dev/null
  ninja -C build-mt bb >/dev/null )
"$SRC/build-mt/bin/bb" verify --scheme chonk -p "$ROOT/out/chonk/flows/$F/proof" -k "$ROOT/out/chonk/vk" \
  --print_bench 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | tee "$HERE/verify-split.txt"
