#!/usr/bin/env bash
# #62 step 2: Barretenberg 6.0.0's CHONK verifier, its C++ unchanged, built for the PVM as the
# service out/chonk-pvm-service.jam. No RISC-V toolchain is installed; the pipeline is
#   Apple clang (riscv64, rv64emac, lp64e) -> LLVM IR -> rustup's llc (LLVM 20) -> objects
#   -> libchonkbb.a -> service/ (Rust, jamswap's jam080-build: rust-lld, polkavm-linker).
# The sysroot is the macOS SDK's libc++ 21 headers with sysroot/cxx/__config_site, picolibc
# 1.8.10's headers, and sysroot/c (picolibc.h, pthread.h and sys/mman.h declarations).
# prelude.h is included first in every file: what bb's common/net.hpp and mem.hpp give Linux
# and wasm (byte order, aligned_free), so bb's sources need no change. What the PVM lacks is
# made up outside bb:
#   - static constructors: ctors.py replaces each file's @llvm.global_ctors with an exported
#     function, and init.cpp (generated) calls them all once from chonk_pvm_init();
#   - thread_local: emulated (-femulated-tls), with rt.cpp's __emutls_get_address;
#   - the C/C++ runtime: rt.cpp (new/delete, std::string, shared_ptr's count, guards, ...),
#     the service (malloc and friends, abort), and stubs.cpp, generated from the symbols the
#     link still lacks, each logging its name and stopping (logging, exceptions, files).
# gk105.cpp routes bb's Grumpkin MSM to the proposed call 105 where the net has it (--wrap, in
# service/build.rs); without the call, bb's own MSM runs.
# OPT (default -O3) is the optimisation level for clang and llc. PLAIN_INLINE (default 1;
# empty for bb's own) adds plain-inline.h: bb's forced inlining off, which the 4 MB code limit
# needs. bb's sources are the checkout's commit, exported (git archive), not its working tree. Needs: Xcode's clang, rustup
# nightly-2025-05-10 with llvm-tools, jamswap's jam080-build, and an aztec-packages v6.0.0
# checkout (BB_SRC, default ../bb-v6.0.0) configured once with `cmake --preset default -B
# build-mt` (for msgpack's headers).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
BB=${BB_SRC:-$SUB/bb-v6.0.0}; DEPS=$BB/barretenberg/cpp/build-mt/_deps
OPT=${OPT:--O3}
PLAIN_INLINE=${PLAIN_INLINE-1}
# llc takes -O0..-O3 only; -Os and -Oz reach it as the IR's optsize/minsize attributes.
case $OPT in -Os|-Oz) LLC_OPT=-O2;; *) LLC_OPT=$OPT;; esac
# Each build starts empty: a previous build's stubs object would answer for symbols this one defines.
O=$HERE/out/build${OPT}${PLAIN_INLINE:+-plain}; rm -rf "$O"; mkdir -p "$O/ll" "$O/obj" "$HERE/out/lib" "$HERE/out/dl"
T=$HOME/.rustup/toolchains/nightly-2025-05-10-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
[ -x "$T/llc" ] || { echo "rustup component add llvm-tools --toolchain nightly-2025-05-10" >&2; exit 1; }
[ -d "$DEPS/msgpack-c" ] || (cd "$BB/barretenberg/cpp" && cmake --preset default -B build-mt >/dev/null)
# bb's sources exactly as committed (0219f11, v6.0.0), whatever the checkout's working tree holds
COMMIT=$(git -C "$BB" rev-parse --short HEAD); SRC=$HERE/out/bb-$COMMIT/barretenberg/cpp
[ -d "$SRC/src" ] || { mkdir -p "$HERE/out/bb-$COMMIT"; git -C "$BB" archive HEAD barretenberg/cpp/src | tar -x -C "$HERE/out/bb-$COMMIT"; }

# 1. the sysroot
PICO=picolibc-1.8.10; TAR=$HERE/out/dl/$PICO.tar.xz
[ -f "$TAR" ] || curl -sSLf -o "$TAR" "https://github.com/picolibc/picolibc/releases/download/1.8.10/$PICO.tar.xz"
echo "0ce5ee3bde81256633644d7b3bd25f5a6ccfd8b0590fd4ee543df2c3ede6e27e  $TAR" | shasum -a 256 -c - >/dev/null
[ -d "$HERE/out/dl/$PICO" ] || tar -C "$HERE/out/dl" -xf "$TAR"
P=$HERE/out/dl/$PICO; R=$HERE/out/sysroot; mkdir -p "$R/c" "$R/cxx"
cp -R "$P/newlib/libc/include/." "$R/c/"; cp -R "$P/newlib/libc/machine/riscv/machine/." "$R/c/machine/"
cp "$P/newlib/libc/tinystdio/stdio.h" "$P/newlib/libc/tinystdio/stdio-bufio.h" "$R/c/"
cp -R "$HERE/sysroot/c/." "$R/c/"
cp -R "$(xcrun --show-sdk-path)/usr/include/c++/v1/." "$R/cxx/"; cp "$HERE/sysroot/cxx/__config_site" "$R/cxx/"

# 2. every file to rv64e IR, constructors exported, then to objects
cc() {  # cc SOURCE [extra flags]: one file to $O/ll/<name>.ll
  local src=$1; shift; local out=$O/ll/$(echo "${src#$SRC/}" | tr '/' '_').ll
  clang++ --target=riscv64-unknown-elf -march=rv64emac -mabi=lp64e -std=gnu++20 $OPT -DNDEBUG \
    -D_GNU_SOURCE -DNO_MULTITHREADING -DBB_NO_EXCEPTIONS -DDISABLE_ASM=1 -DNO_PAR_ALGOS \
    -DMSGPACK_NO_BOOST -DMSGPACK_USE_STD_VARIANT_ADAPTOR \
    -nostdinc -nostdinc++ -isystem "$R/cxx" -isystem "$R/c" -isystem "$(clang -print-resource-dir)/include" \
    -I"$SRC/src" -I"$DEPS/msgpack-c/src/msgpack-c/include" -I"$DEPS/tracy-src/public" \
    -fPIC -femulated-tls -ffunction-sections -fdata-sections -fbracket-depth=1024 -fconstexpr-steps=100000000 \
    -Wno-deprecated-declarations -Wno-vla-cxx-extension -include "$HERE/prelude.h" ${PLAIN_INLINE:+-include "$HERE/plain-inline.h"} \
    "$@" -S -emit-llvm "$src" -o "$out" && echo "$out"
}
lower() {  # lower FILE.ll: constructors exported (ids to $O/ctors.list), then an object
  local in=$1 id; id=$(basename "$in" .ll | tr -c 'A-Za-z0-9_\n' _)
  # Apple clang 21's IR names errnomem, which LLVM 20 lacks; dropping such an attribute is
  # conservative. Its unknown target-feature names, llc ignores.
  sed -i '' -E 's/memory\([^)]*errnomem[^)]*\)//g' "$in"
  python3 "$HERE/ctors.py" "$in" "$id" >> "$O/ctors.list"
  "$T/llc" $LLC_OPT -mtriple=riscv64 -mattr=+e,+m,+a,+c -target-abi=lp64e -relocation-model=pic -emulated-tls \
    -function-sections -data-sections -filetype=obj "$in" -o "$O/obj/$(basename "$in" .ll).o" \
    2> >(grep -v "is not a recognized feature for this target" >&2)
}
export -f cc lower; export SRC DEPS O R T OPT LLC_OPT HERE PLAIN_INLINE
: > "$O/ctors.list"
{ grep -v backing_memory "$HERE/bb-sources.txt" | sed "s|^|$SRC/|"; echo "$HERE/shim.cpp"; echo "$HERE/state.cpp"; echo "$HERE/gk105.cpp"; echo "$HERE/rt.cpp"; } \
  | xargs -P 8 -I{} bash -c 'cc {} -fno-exceptions >/dev/null'
cc "$SRC/src/barretenberg/polynomials/backing_memory.cpp" -fexceptions >/dev/null  # has a try block
ls "$O"/ll/*.ll | xargs -P 8 -I{} bash -c 'lower {}'
{ echo "// Generated by build.sh: every file's static constructors, once (the PVM never runs .init_array)."
  sort -u "$O/ctors.list" | sed 's/.*/extern "C" void chonk_ctors_&();/'
  echo 'extern "C" void chonk_pvm_init()'; echo '{'; echo '    static bool done = false;'
  echo '    if (done) return;'; echo '    done = true;'
  sort -u "$O/ctors.list" | sed 's/.*/    chonk_ctors_&();/'; echo '}'; } > "$O/init.cpp"
lower "$(cc "$O/init.cpp" -fno-exceptions)"

# 3. link once to find what is still missing, generate stubs.cpp, link again
link() {
  "$T/llvm-ar" rcs "$HERE/out/lib/libchonkbb.new.a" "$O"/obj/*.o
  mv "$HERE/out/lib/libchonkbb.new.a" "$HERE/out/lib/libchonkbb.a"
  touch "$HERE/service/src/lib.rs"; (cd "$HERE" && "$BUILD" service --out out)
}
link >/dev/null 2>&1 || true
ELF=$HERE/service/target/jam080/riscv64emac-unknown-none-polkavm/release/chonk_pvm_service.elf
"$T/llvm-nm" -u "$ELF" | awk '{print $2}' | grep -vx chonk_trap > "$O/undefined.txt" || true
c++filt < "$O/undefined.txt" > "$O/undefined.demangled.txt"
python3 "$HERE/stubs.py" "$O/undefined.txt" "$O/undefined.demangled.txt" > "$O/stubs.cpp"
lower "$(cc "$O/stubs.cpp" -fno-exceptions)"
link | tail -1
