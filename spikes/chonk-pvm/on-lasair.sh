#!/usr/bin/env bash
# #62: Aztec client proofs (CHONK) checked in refine on a live six-validator lasair net, their
# verdicts accumulated and finalized. The service is build.sh's: bb 6.0.0's own verifier, its
# C++ unchanged, with its Grumpkin MSM through the proposed host call 105 (gk105.cpp). The net is
# nets/jb.sh's jb-lasair6 (lasair main, native) with the curve calls 101-105 active from slot 0
# on every node (lasair's activation flags: an experiment, not GP 0.8.0), and the full spec's
# refine gas, G_R = 5e9, in the chain spec's protocol parameters: tiny's 1e9 is less than one
# proof's check (3.47B with call 105).
#
# Each proof is one work-item: the payload is [mode][tag] and the verifier's input is the item's
# extrinsic, which travels beside the package (a package is held to 200 KiB). By default (#73)
# the CRS is the service's own preimage: one item asks accumulate to solicit it, `jamboree rpc
# provide` hands it to the chain once, and each proof's extrinsic is its proof and vk alone
# (84 KB, mode 0x81). CRS=extrinsic sends the CRS points in every item instead (2.2 MB, mode 1). Refine
# outputs [tag][verdict][the verifier's gas][MSMs done by call 105]; accumulate stores it at
# "v:" ++ tag. The script waits for each verdict to be in a finalized block.
#
#   spikes/chonk-pvm/on-lasair.sh [N]    N valid proofs (default 2), then one tampered, then a
#                                        second distinct Aztec flow (the 7-layer storage proof)
#   TX=dir ...                           #73 step 3 instead: capture-tx.mjs's genesis.bin and
#                                        tx-N.ext, each verified and applied to Aztec's trees
#                                        (mode 0x83) in chain order, then a stale replay, a
#                                        tampered proof, and the trees against Aztec's own ends
#   UP=0 ...                             use the net already up (nets/jb.sh up)
# Needs: build.sh and measure.sh run (out/chonk-pvm-service.jam, out/verify-1.bin), lasair
# built (dune build in ../lasair), and jamboree built (../jamboree).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
export NET=jb-lasair6 LASAIR_JIP8_FROM_SLOT=0 LASAIR_JIP8_105_FROM_SLOT=0 JB_REFINE_GAS=${JB_REFINE_GAS:-5000000000}
JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}; SPEC=$ROOT/out/$NET/spec.json
O=$HERE/out; N=${1:-2}
rpc() { "$JB" rpc --url ws://127.0.0.1:42950 --chain-spec "$SPEC" "$@"; }
finalized() { rpc head | awk '/finalized/{for (i = 1; i < NF; i++) if ($i == "finalized") print $(i + 1)}' | tr -dc 0-9; }

# the verifier's input as an extrinsic, and the same with one proof octet flipped (measure.sh's)
CH=$ROOT/out/chonk/flows
if [ "${CRS:-preimage}" = preimage ]; then
  MODE=81
  [ -f "$O/crs.raw" ] || { echo "no $O/crs.raw: run crs_raw (see crs_raw.cpp)" >&2; exit 1; }
  python3 "$HERE/pv.py" "$CH/ecdsar1+transfer_0_recursions+sponsored_fpc/proof" "$ROOT/out/chonk/vk" "$O/ext-valid.bin"
  python3 "$HERE/pv.py" "$CH/ecdsar1+transfer_0_recursions+sponsored_fpc/proof" "$ROOT/out/chonk/vk" "$O/ext-bad.bin" 40000
else
  MODE=01
python3 - "$O/verify-1.bin" "$O/ext-valid.bin" "$O/ext-bad.bin" <<'PY'
import sys
b = open(sys.argv[1], "rb").read()
assert b[0] == 1, "verify-1.bin is mode 1"
open(sys.argv[2], "wb").write(b[2:])
bad = bytearray(b[2:]); bad[4 + 40000] ^= 1
open(sys.argv[3], "wb").write(bad)
PY
fi
# a second, distinct Aztec flow: the 7-layer storage proof (10 circuits, 128 KB), the heaviest.
# Every flow carries its own hiding VK (public-input counts differ): the transfer's is
# out/chonk/vk, the storage flow's is committed here (bb writes it when proving the pinned
# CI inputs; the AMM flow's VK differs too and is not kept).
if [ "$MODE" = 81 ]; then python3 "$HERE/pv.py" "$CH/ecdsar1+storage_proof_7_layers+sponsored_fpc/proof" "$HERE/storage-hiding.vk" "$O/ext-storage.bin"
else python3 "$HERE/payload.py" "$CH/ecdsar1+storage_proof_7_layers+sponsored_fpc/proof" \
  "$HERE/storage-hiding.vk" "${BB_CRS:-$HOME/.bb-crs}" 1 32768 "$O/ext-storage.bin"; fi

[ "${UP:-1}" = 1 ] && "$ROOT/nets/jb.sh" up
SID=$(rpc deploy "$O/chonk-pvm-service.jam" --fresh | sed -n 's/^SERVICE_ID=//p')
echo "chonk-pvm service: $SID"
if [ "$MODE" = 81 ]; then
  # accumulate solicits the CRS preimage; once the request is in the state, it can be provided
  rpc submit "$SID" 0x40ff --wait --timeout 300 | tail -1
  for _ in $(seq 1 60); do [ "$(rpc read "$SID" crs:solicited)" = 01 ] && break; sleep 2; done
  echo "CRS solicited: $(rpc read "$SID" crs:solicited)"
  rpc provide "$SID" "@$O/crs.raw"
  # until the preimage is in the state refine reads: a reduction-only probe (mode 0x80), a fresh
  # tag each try, whose verdict is 3 while the CRS is missing
  for try in $(seq 1 20); do
    tag=$((0xe0 + try)); rpc submit "$SID" "0x80$(printf %02x $tag)" --extrinsic "@$O/ext-valid.bin" --wait --timeout 300 >/dev/null
    for _ in $(seq 1 150); do v=$(rpc read "$SID" "0x763a$(printf %02x $tag)"); [ "$v" != "(none)" ] && break; sleep 2; done
    echo "CRS probe $try: verdict ${v:0:2}"; [ "${v:0:2}" = 03 ] || break; sleep 6
  done
fi

submit() {  # submit TAG EXTRINSIC: one proof in its own package; then its verdict, once final
  local tag=$1 ext=$2 t0 key value slot
  t0=$(date +%s)
  key=0x763a$(printf %02x "$tag")
  rpc submit "$SID" "0x$MODE$(printf %02x "$tag")" --extrinsic "@$ext" --wait --timeout 300 | tail -2
  for _ in $(seq 1 150); do value=$(rpc read "$SID" "$key"); [ "$value" != "(none)" ] && break; sleep 2; done
  slot=$(rpc head | awk '{for (i = 1; i < NF; i++) if ($i == "best") print $(i + 1)}' | tr -dc 0-9)
  for _ in $(seq 1 150); do [ "$(finalized)" -ge "$slot" ] 2>/dev/null && break; sleep 2; done
  python3 - "$tag" "$value" "$slot" "$(finalized)" "$(( $(date +%s) - t0 ))" <<'PY'
import sys
tag, v, slot, fin, secs = sys.argv[1:]
b = bytes.fromhex(v.removeprefix("0x")) if v != "(none)" else b""
if len(b) < 17:
    print(f"proof {tag}: no verdict ({v})"); sys.exit()
verdict = {1: "VALID", 0: "INVALID", 2: "MALFORMED", 3: "NO CRS"}.get(b[0], b[0])
print(f"proof {tag}: {verdict}, verifier gas {int.from_bytes(b[1:9], 'little'):,}, "
      f"{int.from_bytes(b[9:17], 'little')} MSMs by call 105; stored by slot {slot}, "
      f"finalized {fin} ({'FINAL' if int(fin or 0) >= int(slot) else 'not final yet'}), {secs} s")
PY
}
batch() {  # batch TAG FILE...: proofs checked together in one item (mode 0x82), one batch IPA
  local tag=$1; shift
  python3 "$HERE/batch.py" "$O/batch-$tag.bin" "$@"
  rpc submit "$SID" "0x82$(printf %02x "$tag")" --extrinsic "@$O/batch-$tag.bin" --wait --timeout 300 | tail -1
  local key=0x763a$(printf %02x "$tag") value
  for _ in $(seq 1 150); do value=$(rpc read "$SID" "$key"); [ "$value" != "(none)" ] && break; sleep 2; done
  python3 - "$tag" "$value" <<'PY'
import sys
tag, v = sys.argv[1:]
b = bytes.fromhex(v) if v != "(none)" else b""
if len(b) < 2 or b[0] != 5:
    print(f"batch {tag}: no batch verdict ({v[:16]})"); sys.exit()
n = b[1]; names = {1: "VALID", 0: "INVALID", 4: "BATCH-IPA-FAILED"}
gas = int.from_bytes(b[2 + n:10 + n], "little"); msms = int.from_bytes(b[10 + n:18 + n], "little")
print(f"batch {tag}: {n} proofs " + " ".join(names.get(x, str(x)) for x in b[2:2 + n]) + f", verifier gas {gas:,} ({gas / n:,.0f} a proof), {msms} MSMs by call 105")
PY
}
if [ -n "${TX:-}" ]; then
  # #73 step 3: the trees move only by proven transitions. Genesis snapshots first (mode 0x41,
  # accumulate holds them once), then each captured transaction (mode 0x83): refine verifies the
  # proof against the pinned hiding vk and applies the kernel's outputs to the trees (state.cpp);
  # accumulate advances only from the snapshots it holds.
  [ "$MODE" = 81 ] || { echo "TX needs CRS=preimage" >&2; exit 1; }
  rpc submit "$SID" "0x4101$(python3 -c "import sys; sys.stdout.write(open('$TX/genesis.bin', 'rb').read(192).hex())")" --wait --timeout 300 | tail -1
  for _ in $(seq 1 150); do [ "$(rpc read "$SID" trees)" != "(none)" ] && break; sleep 2; done
  echo "genesis trees held: $(rpc read "$SID" trees | cut -c1-20)…"

  read_tx() {  # read_tx LABEL TAG: the "t:"++tag record, once stored
    local value; local key=0x743a$(printf %02x "$2")
    for _ in $(seq 1 150); do value=$(rpc read "$SID" "$key"); [ "$value" != "(none)" ] && break; sleep 2; done
    python3 - "$1" "$value" <<'PY'
import sys
label, v = sys.argv[1:]
b = bytes.fromhex(v.removeprefix("0x")) if v != "(none)" else b""
names = {1: "APPLIED", 0: "PROOF INVALID", 5: "TRANSITION REFUSED", 2: "MALFORMED", 3: "NO CRS",
         8: "NOT THE HIDING VK", 9: "STALE START"}
if not b: print(f"{label}: no record ({v})"); sys.exit()
line = f"{label}: {names.get(b[0], b[0])}"
if len(b) >= 241:
    line += (f", fee {int.from_bytes(b[193:225], 'big')}, refine gas {int.from_bytes(b[225:233], 'little'):,}, "
             f"{int.from_bytes(b[233:241], 'little')} MSMs by call 105")
print(line)
PY
  }

  tag=0x20; first=""
  for ext in $(ls "$TX"/tx-*.ext | sort -V); do
    [ -n "$first" ] || first=$ext
    rpc submit "$SID" "0x83$(printf %02x "$tag")" --extrinsic "@$ext" --wait --timeout 300 | tail -1
    read_tx "$(basename "$ext")" "$tag"; tag=$((tag + 1))
  done

  # adversarial: the first transaction again (its start snapshots are history: accumulate must
  # refuse), and its proof with one byte flipped (refine must refuse, trees untouched)
  rpc submit "$SID" "0x83$(printf %02x "$tag")" --extrinsic "@$first" --wait --timeout 300 >/dev/null
  read_tx "$(basename "$first") again" "$tag"; tag=$((tag + 1))
  python3 - "$first" "$O/tx-tampered.ext" <<'PY'
import sys
b = bytearray(open(sys.argv[1], "rb").read())
proof_len = int.from_bytes(b[0:4], "little")
b[4 + proof_len - 100] ^= 1   # inside the proof, past the kernel's public inputs
open(sys.argv[2], "wb").write(b)
PY
  rpc submit "$SID" "0x83$(printf %02x "$tag")" --extrinsic "@$O/tx-tampered.ext" --wait --timeout 300 >/dev/null
  read_tx "$(basename "$first") tampered" "$tag"

  # the chain's trees must be the end snapshots Aztec's own prover computed for the last block
  python3 - "$(ls "$TX"/tx-*.state | sort -V | tail -1)" "$(rpc read "$SID" trees)" <<'PY'
import sys
want = open(sys.argv[1], "rb").read()[-192:].hex()
got = sys.argv[2].removeprefix("0x")
print("trees MATCH Aztec's end snapshots" if got == want else f"trees MISMATCH\n want {want}\n got  {got}")
PY
  rpc head; exit 0
fi

for t in $(seq 1 "$N"); do submit "$t" "$O/ext-valid.bin"; done
submit $((N + 1)) "$O/ext-bad.bin"
submit $((N + 2)) "$O/ext-storage.bin"
if [ "$MODE" = 81 ]; then
  batch 32 "$O/ext-valid.bin" "$O/ext-valid.bin"
  batch 33 "$O/ext-valid.bin" "$O/ext-storage.bin" "$O/ext-valid.bin"
  batch 34 "$O/ext-valid.bin" "$O/ext-valid.bin" "$O/ext-storage.bin" "$O/ext-valid.bin"
  batch 35 "$O/ext-valid.bin" "$O/ext-bad.bin"
fi
rpc head
