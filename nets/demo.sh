#!/usr/bin/env bash
# The demo, one word a stage. Runs against jamswap's lasair6 net (nets/lasair6.sh).
#   nets/demo.sh up        start the net, build and deploy both services
#   nets/demo.sh check     A shields 100, A pays B 40 privately, B unshields 40; every value
#                          read back from the chain byte for byte; a replay must change nothing
#   nets/demo.sh wallet    start the wallet backend (client/wallet.py) on localhost:19710
#   nets/demo.sh down      stop the net and wipe the chain
# Public accounts are 32-byte big-endian field elements: A = 1, B = 2. Private keys derive
# from the names A and B (client/privacy.py). Service ids come from out/state/*.json.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd)
L=$HERE/lasair6.sh; CLIENT=$ROOT/client/privacy.py
export CLIENT_STATE=$ROOT/out/client-state.json
sid() { python3 -c "import json;print(json.load(open('$ROOT/out/state/$1.json'))['service_id'])"; }
hex32() { printf '%064x' "$1"; }
le64() { python3 -c "import struct;print(struct.pack('<Q',$1).hex())"; }
le32() { python3 -c "import struct;print(struct.pack('<I',$1).hex())"; }
ctl() { "$L" ctl "$@" 2>&1 | tail -1; }
submit() { WAIT=${WAIT:-300} "$L" ctl submit "$@" 2>&1 | grep -E "^status .*(Ready|Failed)|timeout" | tail -1 | cut -c1-40; }
# submit_until <sid> <payload> <read-sid> <key> <expected> <name>: send the payload and wait for
# the state it must produce; a package that goes Ready but never accumulates (lasair#93) is
# sent again, which is safe: public-balance skips a nonce it has seen (a mint's random nonce,
# a shield's commitment) and the privacy service a nullifier it has seen.
submit_until() {
  local try got
  for try in 1 2 3; do
    echo "  $(submit "$1" "$2")"
    for _ in 1 2 3 4 5 6 7 8 9 10 11 12; do got=$(ctl read "$3" "$4"); [ "$got" = "$5" ] && break; sleep 3; done
    if [ "$got" = "$5" ]; then echo "  ok   $6 = ${got:0:66}"; return 0; fi
    echo "  …    $6 not there after 36 s (got $got); sending the package again ($try of 3)"
  done
  echo "  FAIL $6: wanted $5, got $got"; exit 1
}
read_eq() { # <sid> <key-hex|text> <expected-hex|"(none)"> <name>; retries while the block settles
  local got; for _ in 1 2 3 4 5 6 7 8 9 10; do got=$(ctl read "$1" "$2"); [ "$got" = "$3" ] && break; sleep 3; done
  if [ "$got" = "$3" ]; then echo "  ok   $4 = ${got:0:66}"; else echo "  FAIL $4: wanted $3, got $got"; exit 1; fi
}
field() { grep "^$2 " "$1" | awk '{print $'"$3"'}' | sed 's/^0x//'; }   # <file> <line-prefix> <column>
u64() { local v; v=$(ctl read "$1" "$2"); [ "$v" = "(none)" ] && echo 0 || python3 -c "import struct;print(struct.unpack('<Q',bytes.fromhex('$v'))[0])"; }
INDEXER=${INDEXER:-http://localhost:19700}
sync_client() { # the client's tree from the indexer (one request), or from the chain key by key
  if curl -sf "$INDEXER/state" > "$ROOT/out/leaves.json" 2>/dev/null; then
    "$CLIENT" sync "$ROOT/out/leaves.json"; return
  fi
  local n leaves="" nr; n=$(ctl read "$PV" leaves); [ "$n" = "(none)" ] && n=0000000000000000
  n=$(python3 -c "import struct;print(struct.unpack('<Q',bytes.fromhex('$n'))[0])")
  for ((i=0;i<n;i++)); do leaves="$leaves\"$(ctl read "$PV" "6c6561663a$(le64 $i)")\","; done
  nr=$(ctl read "$PV" root:null); [ "$nr" = "(none)" ] && nr=""
  echo "{\"leaves\": [${leaves%,}], \"null_root\": \"$nr\"}" > "$ROOT/out/leaves.json"
  "$CLIENT" sync "$ROOT/out/leaves.json"
}
case "${1:-}" in
  up)
    "$L" up
    for _ in $(seq 1 60); do "$L" ctl head >/dev/null 2>&1 && break; sleep 3; done   # the reader answers
    "$L" ctl head; rm -f "$ROOT"/out/state/*.json "$CLIENT_STATE"
    "$L" deploy; echo "services: public-balance $(sid public-balance), privacy $(sid privacy)"
    "$L" indexer; "$0" wallet ;;
  wallet)
    [ -f "$ROOT/out/wallet.pid" ] && kill "$(cat "$ROOT/out/wallet.pid")" 2>/dev/null || true
    nohup python3 "$ROOT/client/wallet.py" > "$ROOT/out/wallet.log" 2>&1 & echo $! > "$ROOT/out/wallet.pid"
    for _ in $(seq 1 20); do curl -sf localhost:19710/api/state?user=A >/dev/null 2>&1 && break; sleep 1; done
    echo "wallet: http://localhost:19710/?user=A   (and ?user=B)   log: out/wallet.log" ;;
  down)
    [ -f "$ROOT/out/wallet.pid" ] && kill "$(cat "$ROOT/out/wallet.pid")" 2>/dev/null || true; rm -f "$ROOT/out/wallet.pid"
    "$L" down ;;
  check)
    PB=$(sid public-balance); PV=$(sid privacy); echo "public-balance = service $PB, privacy = service $PV"
    A=$(hex32 1); B=$(hex32 2); mkdir -p "$ROOT/out"; rm -f "$CLIENT_STATE"
    echo "== the client learns the tree from the chain"; sync_client
    N0=$(u64 "$PV" leaves); T0=$(u64 "$PV" transfers); S0=$(u64 "$PV" shielded); U0=$(u64 "$PV" unshielded)
    BA0=$(u64 "$PB" "62616c3a$A"); BB0=$(u64 "$PB" "62616c3a$B")
    echo "  chain before: $N0 leaves, $T0 transfers, shielded $S0, unshielded $U0, bal:A $BA0, bal:B $BB0"
    echo "== A: a deposit note for 100";   "$CLIENT" deposit A 100 | tee "$ROOT/out/deposit.txt"
    C=$(awk '{print $4}' "$ROOT/out/deposit.txt" | sed 's/^0x//')
    echo "== mint 100 to A (public)"
    submit_until "$PB" "00$A$(le64 100)$(openssl rand -hex 32)" "$PB" "62616c3a$A" "$(le64 $((BA0 + 100)))" "bal:A"
    echo "== shield: A burns 100, the deposit travels to the privacy service with commitment C"
    submit_until "$PB" "02$A$(le64 100)$(le32 "$PV")$C" "$PB" "62616c3a$A" "$(le64 $BA0)" "bal:A back where it started"
    read_eq "$PV" "6465703a$C" "$(le64 100)" "dep:C (arrived by deferred transfer)"
    echo "== A proves: claim the deposit, pay B 40, keep 60"
    "$CLIENT" transfer A deposit:0 - B 40 0 0 0 "$ROOT/out/tx1.hex" | tee "$ROOT/out/tx1.txt" | grep -E "payload|unshield"
    submit_until "$PV" "$(cat "$ROOT/out/tx1.hex")" "$PV" "6465703a$C" "(none)" "dep:C consumed"
    read_eq "$PV" root:note "$(field "$ROOT/out/tx1.txt" new_note_root 2)" "root:note = tx1's new root"
    read_eq "$PV" root:null "$(field "$ROOT/out/tx1.txt" new_null_root 2)" "root:null"
    read_eq "$PV" "6e663a$(field "$ROOT/out/tx1.txt" nullifiers 2)" 01 "nf:<deposit commitment>"
    read_eq "$PV" "6e663a$(field "$ROOT/out/tx1.txt" nullifiers 3)" 01 "nf:<dummy input>"
    read_eq "$PV" "6c6561663a$(le64 $N0)" "$(field "$ROOT/out/tx1.txt" note_hashes 2)" "leaf $N0 = B's 40 note"
    read_eq "$PV" "6c6561663a$(le64 $((N0 + 1)))" "$(field "$ROOT/out/tx1.txt" note_hashes 3)" "leaf $((N0 + 1)) = A's 60 change"
    read_eq "$PV" transfers "$(le64 $((T0 + 1)))" "transfers"
    echo "== B proves: spend the 40 note, unshield 40 to public account B on service $PB"
    "$CLIENT" transfer B 1 - B 0 40 2 "$PB" "$ROOT/out/tx2.hex" | tee "$ROOT/out/tx2.txt" | grep -E "payload|unshield"
    submit_until "$PV" "$(cat "$ROOT/out/tx2.hex")" "$PV" root:note "$(field "$ROOT/out/tx2.txt" new_note_root 2)" "root:note = tx2's new root"
    read_eq "$PV" "6e663a$(field "$ROOT/out/tx2.txt" nullifiers 2)" 01 "nf:<B's 40 note>"
    read_eq "$PV" transfers "$(le64 $((T0 + 2)))" "transfers"
    read_eq "$PV" unshielded "$(le64 $((U0 + 40)))" "unshielded"
    read_eq "$PB" "62616c3a$B" "$(le64 $((BB0 + 40)))" "bal:B (40 arrived by deferred transfer)"
    read_eq "$PV" shielded "$(le64 $((S0 + 100)))" "shielded"
    echo "== replay tx2: a valid proof whose nullifiers are spent"
    echo "  $(submit "$PV" "$(cat "$ROOT/out/tx2.hex")")"; sleep 8
    read_eq "$PV" transfers "$(le64 $((T0 + 2)))" "transfers unchanged"
    read_eq "$PB" "62616c3a$B" "$(le64 $((BB0 + 40)))" "bal:B unchanged"
    echo "== what each side holds"; "$CLIENT" notes
    echo "PASS" ;;
  *) sed -n 2,8p "$0"; exit 1 ;;
esac
