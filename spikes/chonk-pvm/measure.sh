#!/usr/bin/env bash
# #62 step 2: Barretenberg 6.0.0's CHONK verifier, its C++ unchanged, run in refine (build.sh),
# against the same shim run natively. One Aztec client proof (spikes/chonk's pinned
# sponsored-fee transfer), four payloads:
#   reduce-1     the per-proof reduction (mode 0), one BN254 and one Grumpkin CRS point;
#   reduce-bad   the same with one proof octet flipped: must be rejected;
#   reduce-crs   the reduction with the 2^15 Grumpkin points (their parsing, for the next);
#   verify-1     the whole verification (mode 1): the reduction and the proof's own 2^15 IPA.
# Three builds: -O3 (bb's forced inlining), -O3 and -Os with PLAIN_INLINE. Gas is lasair's
# refine (bin/jip8_refine; the protocol's gas, the same on any conforming client), with
# JIP8_GAS_LIMIT raised past a package's 5e9 so verify-1 can finish; times are this Mac only.
# Needs: build.sh's needs, spikes/chonk/measure.sh run (out/chonk), bb's CRS in ~/.bb-crs (any
# bb run fetches it), ../lasair built (dune build bin/jip8_refine.exe), and bb's native
# libraries (ninja -C <bb>/barretenberg/cpp/build-mt bb) for the native column.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
O=$HERE/out; CH=$ROOT/out/chonk; F=ecdsar1+transfer_0_recursions+sponsored_fpc; CRS=${BB_CRS:-$HOME/.bb-crs}
REFINE=$SUB/lasair/_build/default/bin/jip8_refine.exe
BB=${BB_SRC:-$SUB/bb-v6.0.0}/barretenberg/cpp
mk() { python3 "$HERE/payload.py" "$CH/flows/$F/proof" "$CH/vk" "$CRS" 1 "$2" "$O/$1.tmp"
       python3 -c "import sys; open(sys.argv[1],'wb').write(bytes([int(sys.argv[2]),int(sys.argv[3])])+open(sys.argv[4],'rb').read())" \
         "$O/$1.bin" "$3" "$4" "$O/$1.tmp"; rm "$O/$1.tmp"; }
mk reduce-1 1 0 1; mk reduce-crs 32768 0 3; mk verify-1 32768 1 4
python3 -c "import sys; b=bytearray(open(sys.argv[1],'rb').read()); b[1]=2; b[2+4+40000]^=1; open(sys.argv[2],'wb').write(b)" "$O/reduce-1.bin" "$O/reduce-bad.bin"

echo "== native, the same shim (bb from build-mt, one thread)"
NATIVE=$O/native_shim
clang++ -DDISABLE_ASM=1 -DMSGPACK_NO_BOOST -DMSGPACK_USE_STD_VARIANT_ADAPTOR -DNO_PAR_ALGOS -I"$BB/src" \
  -I"$BB/build-mt/_deps/msgpack-c/src/msgpack-c/include" -I"$BB/build-mt/_deps/tracy-src/public" -O3 -DNDEBUG \
  -std=gnu++20 -fbracket-depth=1024 -fconstexpr-steps=100000000 -Wno-deprecated-declarations -Wl,-dead_strip \
  "$HERE/shim.cpp" "$HERE/native_main.cpp" "$BB"/build-mt/lib/*.a -o "$NATIVE"
for p in reduce-1 reduce-bad reduce-crs verify-1; do
  python3 -c "import sys; open(sys.argv[2],'wb').write(open(sys.argv[1],'rb').read()[2:])" "$O/$p.bin" "$O/$p.native"
  mode=$(python3 -c "import sys; print(open(sys.argv[1],'rb').read()[0])" "$O/$p.bin")
  printf '%s ' "$p"; HARDWARE_CONCURRENCY=1 "$NATIVE" "$O/$p.native" "$mode" 20 | tail -1; rm "$O/$p.native"
done

echo "== in refine"
for v in "-O3:" "-O3:1" "-Os:1"; do
  OPT=${v%%:*} PLAIN_INLINE=${v#*:} "$HERE/build.sh" | tail -1
  cp "$O/chonk-pvm-service.jam" "$O/chonk-pvm${v%%:*}${v#*:}.jam"
  echo "-- OPT=${v%%:*} PLAIN_INLINE=${v#*:}"
  JIP8_GAS_LIMIT=40000000000 "$REFINE" "$O/chonk-pvm${v%%:*}${v#*:}.jam" \
    "$O/reduce-1.bin" "$O/reduce-bad.bin" "$O/reduce-crs.bin" "$O/verify-1.bin" | grep -E '^\| [rv].*\| off \|'
done
