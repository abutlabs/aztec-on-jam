#!/usr/bin/env bash
# Build the spike with jamswap's jam080-build and meter one UltraHonk verification in
# refine on lasair's in-process rig (bin/demo_deploy.exe), plain and zk flavours.
#   spikes/ultrahonk-gas/measure.sh            (paths assume Aiden/submodules layout)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
SUB=$(cd "$HERE/../../.." && pwd)
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
DEMO=$SUB/lasair/_build/default/bin/demo_deploy.exe
OUT=${OUT:-$HERE/out}; mkdir -p "$OUT"
[ -x "$BUILD" ] || (cd "$SUB/jamswap/tools/jam080" && cargo build --release -q --manifest-path builder/Cargo.toml)
[ -x "$DEMO" ] || (cd "$SUB/lasair" && { eval "$(opam env 2>/dev/null)" || true; } && dune build bin/demo_deploy.exe)
JAM=$("$BUILD" "$HERE/service" --out "$OUT" | awk '{print $1}')
ls -l "$JAM" | awk '{print "blob bytes", $5}'
refine() { # <payload-hex> -> "<gas> <output-hex> <seconds>"
  local t0 t1; t0=$(date +%s.%N)
  local r; r=$(cd "$SUB/lasair" && LASAIR_REFINE_GAS=${LASAIR_REFINE_GAS:-5000000000} "$DEMO" "$JAM" "$1" | awk '
    /^\[3\] REFINE/ { r = 1 }
    r && /output/   { out = $3 }
    r && /gas used/ { print $4, out; exit }')
  t1=$(date +%s.%N); echo "$r $(echo "$t1 - $t0" | bc)"
}
echo "| flavour | gas | output | lasair seconds |"; echo "|---|---|---|---|"
read -r g o s < <(refine 00); printf '| plain (evm-no-zk) | %s | %s | %s |\n' "$(printf "%'d" "$g")" "$o" "$s"
read -r g o s < <(refine 01); printf '| zk (evm) | %s | %s | %s |\n' "$(printf "%'d" "$g")" "$o" "$s"
read -r g o s < <(refine 02); printf '| zk, one proof byte flipped (must be 00) | %s | %s | %s |\n' "$(printf "%'d" "$g")" "$o" "$s"
read -r g o s < <(refine 03); printf '| zk, wrong public input (must be 00) | %s | %s | %s |\n' "$(printf "%'d" "$g")" "$o" "$s"
