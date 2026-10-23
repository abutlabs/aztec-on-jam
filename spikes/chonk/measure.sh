#!/usr/bin/env bash
# #18's spike: Aztec v6's own private kernel circuits, with Aztec v6's toolchain (Noir 1.0.0-rc.3,
# Barretenberg 6.0.0), against our circuit and verifier.
#   1. Aztec CI's pinned benchmark inputs (11 flows: each the app circuits and the private
#      kernels of one transaction, bytecode and witnesses): proved with `bb prove --scheme chonk`.
#   2. One proof's hiding-kernel key written, the proof verified, then timed on one thread
#      against our UltraHonk proof (scripts/offline-proof.sh's) on one thread: the ratio,
#      times our verifier's refine gas, estimates a CHONK verifier's refine gas.
#   3. The kernels compiled from Aztec's v6.0.0 source with nargo rc.3, sizes against the stack's.
# Needs: bb 6.0.0 at ~/.bb/versions/bb-6.0.0 (bbup -v 6.0.0), nargo 1.0.0-rc.3 at
# ~/.nargo/versions/nargo-1.0.0-rc.3 (noirup -v 1.0.0-rc.3), node + npm, python3 with msgpack,
# and out/offline from scripts/offline-proof.sh.  Writes out/chonk/.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd); O=$ROOT/out/chonk; mkdir -p "$O"
BB6=$HOME/.bb/versions/bb-6.0.0; BB5=$HOME/.bb/versions/bb-5.0.0; NARGO=$HOME/.nargo/versions/nargo-1.0.0-rc.3
HASH=afe8389cb7f21295            # aztec-packages v6.0.0, barretenberg/cpp/scripts/chonk-inputs.hash
IN=$HOME/.cache/jam-privacy/chonk-inputs-$HASH
REFINE_GAS=169112761             # our verifier in refine (services/privacy/check.sh)

if [ ! -d "$IN" ]; then
  mkdir -p "$IN" && curl -sfL "https://aztec-ci-artifacts.s3.us-east-2.amazonaws.com/protocol/bb-chonk-inputs-$HASH.tar.gz" | tar xz -C "$IN" --strip-components=1
fi

echo "== 1. every flow, proved (bb 6.0.0, $(sysctl -n hw.ncpu 2>/dev/null || nproc) threads)"
for f in $(ls "$IN" | grep -v joblog); do
  mkdir -p "$O/flows/$f"
  /usr/bin/time -p "$BB6" prove --scheme chonk --ivc_inputs_path "$IN/$f/ivc-inputs.msgpack" -o "$O/flows/$f" > "$O/flows/$f/log" 2>&1
  n=$(grep -oE "generating proof for [0-9]+ accumulated" "$O/flows/$f/log" | grep -oE "[0-9]+")
  printf '%-62s %2s circuits  %6s s  %7s bytes\n' "$f" "$n" "$(awk '/^real/{print $2}' "$O/flows/$f/log")" "$(wc -c < "$O/flows/$f/proof" | tr -d ' ')"
done

echo "== 2. verification on one thread (the PVM has one)"
F=ecdsar1+transfer_0_recursions+sponsored_fpc
python3 - "$IN/$F/ivc-inputs.msgpack" "$O/hiding.bytecode" <<'PY'
import msgpack, sys
stack = msgpack.unpackb(open(sys.argv[1], "rb").read(), raw=False, strict_map_key=False)
open(sys.argv[2], "wb").write(stack[-1]["bytecode"])
PY
"$BB6" write_vk --scheme chonk --circuit_kind hiding -b "$O/hiding.bytecode" -o "$O" >/dev/null 2>&1
"$BB6" verify --scheme chonk -p "$O/flows/$F/proof" -k "$O/vk" >/dev/null 2>&1 && echo "the $F proof verifies"
export HARDWARE_CONCURRENCY=1
cpu() { /usr/bin/time -p bash -c "for i in \$(seq 1 $1); do $2 >/dev/null 2>&1; done" 2>&1 | awk '/^user/{print $2}'; }
H=200; C=20
uh=$(cpu $H "$BB5 verify -t evm -p $ROOT/out/offline/proof -i $ROOT/out/offline/public_inputs -k $ROOT/services/privacy/fixtures/transfer.vk")
uh0=$(cpu $H "$BB5 --version")
ch=$(cpu $C "$BB6 verify --scheme chonk -p '$O/flows/$F/proof' -k $O/vk")
ch0=$(cpu $C "$BB6 --version")
python3 - "$uh" "$uh0" "$ch" "$ch0" "$H" "$C" "$REFINE_GAS" <<'PY'
import sys
uh, uh0, ch, ch0, H, C, gas = map(float, sys.argv[1:])
u = (uh - uh0) / H; c = (ch - ch0) / C; r = c / u
print(f"UltraHonk (ours, 8,768-byte proof): {u*1000:.1f} ms CPU; CHONK (Aztec's, 80,096 bytes): {c*1000:.0f} ms; ratio {r:.0f}x")
print(f"estimated CHONK verifier in refine: {gas*r/1e9:.1f} billion gas (a full package holds 5, a tiny one 1)")
PY

echo "== 3. the kernels from Aztec's v6.0.0 source, compiled with nargo 1.0.0-rc.3"
A=$O/aztec-v6
if [ ! -d "$A/noir-projects" ]; then
  rm -rf "$A"; git init -q "$A"; git -C "$A" remote add origin https://github.com/AztecProtocol/aztec-packages
  git -C "$A" config core.sparseCheckout true; echo 'noir-projects/fnd/noir-protocol-circuits/' > "$A/.git/info/sparse-checkout"
  git -C "$A" fetch -q --depth 1 origin tag v6.0.0; git -C "$A" checkout -q FETCH_HEAD
fi
W=$A/noir-projects/fnd/noir-protocol-circuits
(cd "$W" && [ -f Nargo.toml ] || { npm install --silent >/dev/null 2>&1; node scripts/generate_variants.js --test >/dev/null; node scripts/generate_variants.js >/dev/null; })
for p in private_kernel_init_3 private_kernel_reset_tail hiding_kernel_to_rollup; do
  (cd "$W" && "$NARGO" compile --package $p --skip-brillig-constraints-check --silence-warnings >/dev/null 2>&1)
  python3 -c "import json,base64,sys; open(sys.argv[2],'wb').write(base64.b64decode(json.load(open(sys.argv[1]))['bytecode']))" "$W/target/$p.json" "$O/$p.bytecode"
  printf '%-26s %s\n' "$p" "$("$BB6" gates --scheme chonk -b "$O/$p.bytecode" 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin)["functions"][0]; print("%s opcodes, %s gates" % (format(d["acir_opcodes"], ","), format(d["circuit_size"], ",")))')"
done
