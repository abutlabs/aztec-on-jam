#!/usr/bin/env bash
# Call 105's vectors (lasair#130) checked by Barretenberg's own Grumpkin, the consumer's library
# and a third implementation beside e2e's two: `e2e --vectors` | this. Needs an aztec-packages
# v6.0.0 checkout (BB_SRC, default ../../../../bb-v6.0.0) with its build-mt libraries built.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
B=${BB_SRC:-$HERE/../../../../bb-v6.0.0}/barretenberg/cpp
O=$HERE/../out; mkdir -p "$O"
clang++ -O2 -std=gnu++20 -DDISABLE_ASM=1 -DNO_PAR_ALGOS -I"$B/src" -I"$B/build-mt/_deps/msgpack-c/src/msgpack-c/include" \
  -I"$B/build-mt/_deps/tracy-src/public" -fbracket-depth=1024 -Wno-deprecated-declarations \
  "$HERE/check-105.cpp" -Wl,-dead_strip "$B"/build-mt/lib/*.a -o "$O/bb-check-105"
python3 -c '
import json, sys
for l in sys.stdin:
    d = json.loads(l)
    if d["call"] == "grumpkin_g1_msm":
        print(d["result"], d["input"] or "-", d.get("output") or "-")' | "$O/bb-check-105"
