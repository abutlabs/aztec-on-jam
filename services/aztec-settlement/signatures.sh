#!/usr/bin/env bash
# #54: the proposer's signature on a PolkaJam testnet. cfg names a committee of four test keys;
# Aztec v6's released code signs checkpoints as its validators do (tools/aztec-gen/gen.mjs: the
# payload digest by every member but one, the attestations-and-signers digest by the slot's
# proposer). Sent: checkpoint 1 signed (accepted); checkpoint 2 with the proposer unsigned, with
# the proposer's signature corrupted, and with no attestations (all refused in refine); then
# checkpoint 2 signed (accepted).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out/signatures; G=$O/cps; mkdir -p "$O"
. "$HERE/../../spikes/lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
GEN=$HERE/../../tools/aztec-gen; T=$HERE/testlib.py
[ -d "$GEN/node_modules" ] || (cd "$GEN" && npm install --omit=dev --no-audit --no-fund > /dev/null)
GENESIS=$(( $(date +%s) - 3600 ))
KEYS='"0x0000000000000000000000000000000000000000000000000000000000001001", "0x0000000000000000000000000000000000000000000000000000000000001002", "0x0000000000000000000000000000000000000000000000000000000000001003", "0x0000000000000000000000000000000000000000000000000000000000001004"'
rm -rf "$G"; mkdir -p "$G"
(cd "$GEN" && node -e "
import('@aztec-labs/foundation/crypto/secp256k1-signer').then(async ({ Secp256k1Signer }) => {
  const { Buffer32 } = await import('@aztec-labs/foundation/buffer');
  const keys = [$KEYS].map((h) => new Secp256k1Signer(Buffer32.fromString(h)));
  require('fs').writeFileSync('$G/committee.json', JSON.stringify({ addresses: keys.map((k) => k.address.toString()), seed: '12345' }));
  process.exit(0);
})")
cat > "$G/spec.json" <<JSON
{"genesis": $GENESIS, "slotSeconds": 6, "fee": 1000, "genesisArchive": null,
 "committee": {"keys": [$KEYS], "seed": "12345", "epochSlots": 32, "chainId": 31337, "rollup": "0x00000000000000000000000000000000a11ce002"},
 "checkpoints": [
 {"name": "c1", "parent": "genesis", "slot": 1},
 {"name": "c2", "parent": "c1", "slot": 2},
 {"name": "c2nop", "parent": "c1", "slot": 2, "sign": "noProposer"},
 {"name": "c2bad", "parent": "c1", "slot": 2, "sign": "badSig"},
 {"name": "c2none", "parent": "c1", "slot": 2}]}
JSON
(cd "$GEN" && node gen.mjs "$G" "$G/spec.json" > /dev/null)
rm "$G/c2none.attest"                                     # sent without attestations
"$T" cfg "$G" "$GENESIS" "$HERE/cfg.bin" - "$G/committee.json"
"$T" payload 1 "$G" c1 0 > "$G/p1.bin"
"$T" payload 2 "$G" c2nop 0 > "$G/p2.bin"
"$T" payload 3 "$G" c2bad 0 > "$G/p3.bin"
"$T" payload 4 "$G" c2none 0 > "$G/p4.bin"
"$T" payload 5 "$G" c2 0 > "$G/p5.bin"
BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
xs() { "$T" extrinsics "$G" "$1"; }
pj_send 1 "$G/p1.bin" $(xs c1)
pj_send 2 "$G/p2.bin" $(xs c2nop)
pj_send 3 "$G/p3.bin" $(xs c2bad)
pj_send 4 "$G/p4.bin" $(xs c2none)
pj_send 5 "$G/p5.bin" $(xs c2)
sleep 12
names=("" "checkpoint 1, signed" "checkpoint 2, the proposer unsigned" "checkpoint 2, the proposer's signature corrupted" "checkpoint 2, no attestations" "checkpoint 2, signed")
want=("" 1 6 6 6 1)
echo
echo "| case | refine gas | verdict | expected |"
echo "|---|---|---|---|"
for t in 1 2 3 4 5; do
  g=$(pj_refines "$t" | awk '{print $1}' | sort -n | head -1)
  v=$(pj_read "$(printf '763a%02x' "$t")" | tr -d '"')
  printf '| %s | %s | %s | %s |\n' "${names[$t]}" "$(python3 -c "print(f'{int(\"${g:-0}\"):,}')")" "$(python3 -c "import sys; v = sys.argv[1]; print(int(v, 16) if v and all(c in '0123456789abcdef' for c in v) else 'none: ' + v)" "$v")" "${want[$t]}"
done
