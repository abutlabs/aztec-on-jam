#!/usr/bin/env bash
# #50: the settlement service's Inbox on a PolkaJam testnet. A second service
# (spikes/inbox-sender) sends L1→L2 messages as deferred transfers: 3 in one package, then 2 in
# another, one of them for another rollup version (refused). The buckets' rolling hashes and the
# leaves must equal what Aztec v6's released code computes for the same messages
# (tools/aztec-gen/inbox.mjs). Then checkpoints from Aztec's packages consume them: one skipping
# the second bucket past its cutoff, one consuming both, then from that one: behind it, the
# wrong hash for its bucket, a bucket that doesn't exist, and one consuming nothing new.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out/inbox; G=$O/cps; mkdir -p "$O"
. "$HERE/../../spikes/lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
GEN=$HERE/../../tools/aztec-gen; T=$HERE/testlib.py
[ -d "$GEN/node_modules" ] || (cd "$GEN" && npm install --omit=dev --no-audit --no-fund > /dev/null)
GENESIS=$(( $(date +%s) - 600 ))
rm -rf "$G"; mkdir -p "$G"
echo '{"genesis": 0, "slotSeconds": 6, "fee": 1000, "genesisArchive": null, "checkpoints": []}' > "$G/g.json"
(cd "$GEN" && node gen.mjs "$G" "$G/g.json" > /dev/null)        # just the genesis archive
"$T" cfg "$G" "$GENESIS" "$HERE/cfg.bin"
SETTLE_BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
SENDER_BLOB=$("$BUILD" "$HERE/../../spikes/inbox-sender/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$SETTLE_BLOB"; SETTLE=$PJ_SID
pj_deploy "$SENDER_BLOB"; SENDER=$PJ_SID; PJ_SID=$SETTLE   # reads and checkpoints go to the settlement service
echo "settlement service $SETTLE, sender $SENDER"
RECIPIENT=0x1647b194c649f5dd01d7c832f89b0f496043c9150797923ea89e93d5ac619a93   # the fixtures' recipient
python3 - "$O" "$SETTLE" "$SENDER" "$RECIPIENT" <<'PY'
import json, sys
out, settle, sender, recipient = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
w = lambda x: int(x, 16).to_bytes(32, "big") if isinstance(x, str) else x.to_bytes(32, "big")
memo = lambda m, version=1: w(m["recipient"]) + w(m["content"]) + w(m["secretHash"]) + w(version)
a = [{"recipient": recipient, "content": c, "secretHash": 0} for c in (1, 2, 3)]
b = [{"recipient": 5, "content": "0x2a", "secretHash": "0x0123456789"}]
pkg = lambda ms: settle.to_bytes(4, "little") + bytes([len(ms)]) + b"".join(ms)
open(f"{out}/m1.bin", "wb").write(pkg([memo(m) for m in a]))
open(f"{out}/m2.bin", "wb").write(pkg([memo(b[0], version=2), memo(b[0])]))   # the first is for version 2
json.dump({"chainId": 31337, "version": 1, "messages": [dict(m, sender=sender) for m in a + b], "buckets": [3, 1]},
          open(f"{out}/inbox-spec.json", "w"))
PY
PJ_SID=$SENDER pj_send m1 "$O/m1.bin"
PJ_SID=$SENDER pj_send m2 "$O/m2.bin"
sleep 8
(cd "$GEN" && node inbox.mjs "$O/inbox-spec.json") > "$O/inbox-expected.json"
rd() { pj_read "$1" | tr -d '"'; }
hexkey() { python3 -c "import sys; print(sys.argv[1].encode().hex() + int(sys.argv[2]).to_bytes(8, 'little').hex())" "$1" "$2"; }
B1=$(rd "$(hexkey inbox:b: 1)"); B2=$(rd "$(hexkey inbox:b: 2)")
python3 - "$O/inbox-expected.json" "$B1" "$B2" "$(rd "$(python3 -c "print(b'inbox:seq'.hex())")")" "$(rd "$(python3 -c "print(b'inbox:refused'.hex())")")" \
  "$(for i in 0 1 2 3; do rd "$(hexkey inbox:m: $i)" | cut -c1-64; done | tr '\n' ' ')" "$O/buckets.json" <<'PY'
import json, sys
want = json.load(open(sys.argv[1])); b1, b2, seq, refused, leaves, out = sys.argv[2:8]
ok = lambda c: "equals" if c else "DIFFERS from"
for i, b in enumerate((b1, b2)):
    print(f"bucket {i + 1}: rolling hash {ok(b[:64] == want['buckets'][i]['rollingHash'][2:])} Aztec's, total {int.from_bytes(bytes.fromhex(b[64:80]), 'little')} (Aztec {want['buckets'][i]['total']})")
print(f"buckets {int.from_bytes(bytes.fromhex(seq), 'little')}, refused messages {int.from_bytes(bytes.fromhex(refused), 'little')}")
for i, l in enumerate(leaves.split()):
    print(f"message {i}: leaf {ok(l == want['leaves'][i][2:])} Aztec's")
json.dump({"b1": {"hash": "0x" + b1[:64], "time": int.from_bytes(bytes.fromhex(b1[80:96]), "little")},
           "b2": {"hash": "0x" + b2[:64], "time": int.from_bytes(bytes.fromhex(b2[80:96]), "little")}}, open(out, "w"))
PY
# a slot whose cutoff is past bucket 2's time, so skipping it is refused; wait until it is current
S1=$(python3 -c "import json, math; b = json.load(open('$O/buckets.json')); print(math.ceil((b['b2']['time'] + 6 - $GENESIS) / 6) + 2)")
while [ $(( ($(date +%s) - GENESIS) / 6 )) -lt $((S1 + 1)) ]; do sleep 2; done
H1=$(python3 -c "import json; print(json.load(open('$O/buckets.json'))['b1']['hash'])")
H2=$(python3 -c "import json; print(json.load(open('$O/buckets.json'))['b2']['hash'])")
cat > "$G/spec.json" <<JSON
{"genesis": $GENESIS, "slotSeconds": 6, "fee": 1000, "genesisArchive": "$(python3 -c "import json; print(json.load(open('$G/genesis.json'))['archive'])")", "checkpoints": [
 {"name": "skip", "parent": "genesis", "slot": $S1, "inbox": "$H1"},
 {"name": "a", "parent": "genesis", "slot": $S1, "inbox": "$H2"},
 {"name": "behind", "parent": "a", "slot": $((S1 + 1)), "inbox": "$H1"},
 {"name": "b", "parent": "a", "slot": $((S1 + 1)), "inbox": "$H2"}]}
JSON
(cd "$GEN" && node gen.mjs "$G" "$G/spec.json")
"$T" payload 1 "$G" skip 1 > "$G/p1.bin"
"$T" payload 2 "$G" a 2 > "$G/p2.bin"
"$T" payload 3 "$G" behind 1 > "$G/p3.bin"
"$T" payload 4 "$G" b 2 "inbox=$H1" > "$G/p4.bin"          # bucket 2 named, bucket 1's hash
"$T" payload 5 "$G" b 9 > "$G/p5.bin"                      # no bucket 9
"$T" payload 6 "$G" b 2 > "$G/p6.bin"
xs() { "$T" extrinsics "$G" "$1"; }
pj_send 1 "$G/p1.bin" $(xs skip)
pj_send 2 "$G/p2.bin" $(xs a)
pj_send 3 "$G/p3.bin" $(xs behind)
pj_send 4 "$G/p4.bin" $(xs b)
pj_send 5 "$G/p5.bin" $(xs b)
pj_send 6 "$G/p6.bin" $(xs b)
sleep 12
names=("" "skips bucket 2, past its cutoff" "consumes buckets 1-2" "then bucket 1 (behind the parent)" "bucket 2 named, bucket 1's hash" "bucket 9, which doesn't exist" "then bucket 2 (nothing new)")
want=("" 20 1 18 15 16 1)
echo
echo "| checkpoint | verdict | expected |"
echo "|---|---|---|"
for t in 1 2 3 4 5 6; do
  v=$(rd "$(printf '763a%02x' "$t")")
  printf '| %s | %s | %s |\n' "${names[$t]}" "$(python3 -c "import sys; v = sys.argv[1]; print(int(v, 16) if v and all(c in '0123456789abcdef' for c in v) else 'none: ' + v)" "$v")" "${want[$t]}"
done
for i in 1 2; do
  log=$(rd "$(hexkey cp: $i)")
  echo "cp:$i consumed $(python3 -c "import sys; print(int.from_bytes(bytes.fromhex(sys.argv[1][336:352]), 'little'))" "$log") messages, bucket $(python3 -c "import sys; print(int.from_bytes(bytes.fromhex(sys.argv[1][352:368]), 'little'))" "$log")"
done
