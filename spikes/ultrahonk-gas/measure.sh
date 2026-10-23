#!/usr/bin/env bash
# Meter one UltraHonk verification in refine on lasair's in-process rig, for bb 3.0.3 and
# bb 5.0.0 proofs of the same circuit, zk and plain, with two negative controls each.
#   spikes/ultrahonk-gas/measure.sh            (paths assume the Aiden/submodules layout)
# Needs ~/.bb/versions/bb-{3.0.3,5.0.0} and ~/.nargo/versions/nargo-{1.0.0-beta.16,1.0.0-rc.5}
# (scripts/bootstrap.sh installs them).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
SUB=$(cd "$HERE/../../.." && pwd)
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
DEMO=$SUB/lasair/_build/default/bin/demo_deploy.exe
OUT=${OUT:-$HERE/out}; mkdir -p "$OUT"
V=$HOME; BB3=$V/.bb/versions/bb-3.0.3; BB5=$V/.bb/versions/bb-5.0.0
N3=$V/.nargo/versions/nargo-1.0.0-beta.16; N5=$V/.nargo/versions/nargo-1.0.0-rc.5
[ -x "$BUILD" ] || (cd "$SUB/jamswap/tools/jam080" && cargo build --release -q --manifest-path builder/Cargo.toml)
[ -x "$DEMO" ] || (cd "$SUB/lasair" && { eval "$(opam env 2>/dev/null)" || true; } && dune build bin/demo_deploy.exe)

artifacts() { # <bb3|bb5>: prove the circuit zk and plain with that toolchain
  local tag=$1 nargo bb dir; dir=$HERE/artifacts/$tag
  if [ "$tag" = bb3 ]; then nargo=$N3; bb=$BB3; else nargo=$N5; bb=$BB5; fi
  rm -rf "$HERE/circuit/target" "$dir"; mkdir -p "$dir/zk" "$dir/plain"
  (cd "$HERE/circuit" && "$nargo" execute >/dev/null)
  if [ "$tag" = bb3 ]; then
    (cd "$HERE/circuit" && "$bb" prove -s ultra_honk --oracle_hash keccak -b target/circuit.json -w target/circuit.gz -o "$dir/zk" --write_vk >/dev/null 2>&1
                         "$bb" prove -s ultra_honk --oracle_hash keccak --disable_zk -b target/circuit.json -w target/circuit.gz -o "$dir/plain" --write_vk >/dev/null 2>&1)
  else
    for f in zk:evm plain:evm-no-zk; do
      (cd "$HERE/circuit" && "$bb" write_vk -t "${f#*:}" -b target/circuit.json -o "$dir/${f%%:*}" >/dev/null 2>&1 &&
                           "$bb" prove -t "${f#*:}" -b target/circuit.json -w target/circuit.gz -k "$dir/${f%%:*}/vk" -o "$dir/${f%%:*}" >/dev/null 2>&1)
    done
  fi
}

refine() { # <jam> <payload-hex> -> "<gas> <output-hex> <steps>"
  (cd "$SUB/lasair" && LASAIR_REFINE_GAS=${LASAIR_REFINE_GAS:-5000000000} "$DEMO" "$1" "$2") | awk '
    /^\[3\] REFINE/ { r = 1 }
    r && /output/   { out = $3 }
    r && /gas used/ { print $4, out, $6; exit }'
}

echo "| bb | case | gas | output | proof bytes | steps |"; echo "|---|---|---|---|---|---|"
for tag in bb3 bb5; do
  artifacts "$tag"
  # jam080-build takes no cargo flags: pick the verifier through the crate's default feature
  cp "$HERE/service/Cargo.toml" "$OUT/Cargo.toml.orig"
  sed -i '' -e "s/^default = .*/default = [\"$tag\"]/" "$HERE/service/Cargo.toml" 2>/dev/null || sed -i -e "s/^default = .*/default = [\"$tag\"]/" "$HERE/service/Cargo.toml"
  JAM=$("$BUILD" "$HERE/service" --out "$OUT/$tag" | awk '{print $1}'); cp "$OUT/Cargo.toml.orig" "$HERE/service/Cargo.toml"
  for c in "00:plain" "01:zk" "02:zk, proof byte flipped (must be 00)" "03:zk, wrong public input (must be 00)"; do
    read -r g o s < <(refine "$JAM" "${c%%:*}")
    pb=$(wc -c < "$HERE/artifacts/$tag/$( [ "${c%%:*}" = 00 ] && echo plain || echo zk )/proof" | tr -d ' ')
    printf '| %s | %s | %s | %s | %s | %s |\n' "$tag" "${c#*:}" "$(printf "%'d" "$g")" "$o" "$pb" "$s"
  done
done
