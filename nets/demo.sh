#!/usr/bin/env bash
# The demo, one word a stage, on a JAM tiny net: NET=lasair6 (default) or NET=pj6 (nets/net.sh).
#   nets/demo.sh up         six validators, both services deployed, the indexer and the wallet
#   nets/demo.sh check      no browser: A shields 100, A sends 40 to B, B unshields 40; asserts
#                           the chain byte for byte; a replay of the send must change nothing
#   nets/demo.sh shield     A shields 100 (minting first if A is short)  (args: user amount)
#   nets/demo.sh send       A sends 40 to B privately; B finds it by scanning  (args: from to amount)
#   nets/demo.sh unshield   B unshields 40 to B's public account   (args: user amount)
#   nets/demo.sh wallet     (re)start the wallet page's static server, http://localhost:19710/?user=A
#                           (the page keeps its own keys and proves in the browser)
#   nets/demo.sh down       stop everything and wipe the chain
# Public accounts are 32-byte big-endian field elements: A = 1, B = 2. Private keys derive
# from the names (client/privacy.py). Service ids come from out/state/*.json.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd)
L=$HERE/net.sh; export NET=${NET:-lasair6}; CLIENT=$ROOT/client/privacy.py
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
    "$L" ctl head; rm -f "$ROOT"/out/state/*.json "$CLIENT_STATE" "$ROOT"/out/wallet-state.json "$ROOT"/out/wallet-state.bak
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
    A=$(hex32 1); B=$(hex32 2); mkdir -p "$ROOT/out"; T=$ROOT/out
    # A and B are separate clients: separate note state, sharing nothing but the chain
    SA=$T/client-A.json; SB=$T/client-B.json; rm -f "$SA" "$SB" "$T/keys.json"
    asA() { CLIENT_STATE=$SA PRIVACY_KEYS=$T/keys.json "$CLIENT" "$@"; }
    asB() { CLIENT_STATE=$SB PRIVACY_KEYS=$T/keys.json "$CLIENT" "$@"; }
    echo "== each client learns the tree from the chain"; CLIENT_STATE=$SA sync_client; CLIENT_STATE=$SB sync_client >/dev/null
    N0=$(u64 "$PV" leaves); T0=$(u64 "$PV" transfers); S0=$(u64 "$PV" shielded); U0=$(u64 "$PV" unshielded)
    BA0=$(u64 "$PB" "62616c3a$A"); BB0=$(u64 "$PB" "62616c3a$B"); R0=$(ctl read "$PV" root:note); NR0=$(ctl read "$PV" root:null)
    # a fresh chain holds no roots: both trees are empty (the note tree's zero root, the
    # nullifier tree holding only its zero leaf), as the client computes them
    [ "$R0" = "(none)" ] && R0=$(asA root | sed 's/^0x//')
    [ "$NR0" = "(none)" ] && NR0=$(asA null-root | sed 's/^0x//')   # the nullifier tree's zero leaf
    echo "  before: $N0 leaves, $T0 transfers, shielded $S0, unshielded $U0, bal:A $BA0, bal:B $BB0, root:note ${R0:0:16}…"

    echo "== 0. A and B publish how to pay them (encryption key, spend key) in the directory"
    KA=$(asA directory A); KB=$(asB directory B)
    submit_until "$PB" "03$A$KA" "$PB" "6b65793a$A" "$KA" "key:A"
    submit_until "$PB" "03$B$KB" "$PB" "6b65793a$B" "$KB" "key:B"
    # A's client reads B's key from the chain, not from B
    echo "{\"A\": \"$(curl -s "$INDEXER/key/$A" | python3 -c 'import json,sys;print(json.load(sys.stdin)["key"])')\", \"B\": \"$(curl -s "$INDEXER/key/$B" | python3 -c 'import json,sys;print(json.load(sys.stdin)["key"])')\"}" > "$T/keys.json"

    echo "== 1. shield: A mints 100 publicly, burns it into a deposit, and proves the claim"
    asA deposit A 100 | tee "$T/deposit.txt"; C=$(awk '{print $4}' "$T/deposit.txt" | sed 's/^0x//')
    submit_until "$PB" "00$A$(le64 100)$(openssl rand -hex 32)" "$PB" "62616c3a$A" "$(le64 $((BA0 + 100)))" "bal:A after the mint (+100)"
    submit_until "$PB" "02$A$(le64 100)$(le32 "$PV")$C" "$PB" "62616c3a$A" "$(le64 $BA0)" "bal:A after the shield (-100)"
    read_eq "$PV" "6465703a$C" "$(le64 100)" "dep:C, arrived by deferred transfer"
    asA transfer A deposit:0 - A 100 0 0 0 "$T/tx1.hex" > "$T/tx1.txt"; grep payload "$T/tx1.txt"
    R1=$(field "$T/tx1.txt" new_note_root 2)
    submit_until "$PV" "$(cat "$T/tx1.hex")" "$PV" root:note "$R1" "root:note, change 1 of 3 (the claim)"
    read_eq "$PV" "6465703a$C" "(none)" "dep:C consumed"

    echo "== 2. send: A proves a payment of 40 to B, keeping 60, sealing B's note to B's key"
    asA transfer A 1 - B 40 0 0 0 "$T/tx2.hex" > "$T/tx2.txt"; grep payload "$T/tx2.txt"
    R2=$(field "$T/tx2.txt" new_note_root 2)
    submit_until "$PV" "$(cat "$T/tx2.hex")" "$PV" root:note "$R2" "root:note, change 2 of 3 (the send)"

    echo "== B's client finds the payment by scanning the chain with B's key"
    for _ in 1 2 3 4 5; do curl -sf "$INDEXER/state" > "$T/chain.json"; python3 -c "import json,sys; sys.exit(0 if json.load(open('$T/chain.json'))['note_root']=='$R2' else 1)" && break; sleep 2; done
    asB scan B "$T/chain.json" | tee "$T/scanB.txt"
    NB=$(grep -c "found note .*: B 40 at leaf $((N0 + 2))" "$T/scanB.txt" || true)
    [ "$NB" = 1 ] && [ "$(grep -c 'found note' "$T/scanB.txt")" = 1 ] && echo "  ok   B's client found exactly one note: 40 at leaf $((N0 + 2)), sealed to B on chain" || { echo "  FAIL B's scan: $(cat "$T/scanB.txt")"; exit 1; }
    BNOTE=$(sed -n 's/^found note \([0-9]*\): B 40.*/\1/p' "$T/scanB.txt")

    echo "== 3. unshield: B proves a spend of the found note, 40 to public account B on service $PB"
    asB transfer B "$BNOTE" - B 0 40 2 "$PB" "$T/tx3.hex" > "$T/tx3.txt"; grep -E "payload|unshield" "$T/tx3.txt"
    R3=$(field "$T/tx3.txt" new_note_root 2)
    submit_until "$PV" "$(cat "$T/tx3.hex")" "$PV" root:note "$R3" "root:note, change 3 of 3 (the unshield)"

    echo "== the note root: three changes, each the builder's, and the chain's leaves give it back"
    [ "$R0" != "$R1" ] && [ "$R1" != "$R2" ] && [ "$R2" != "$R3" ] && echo "  ok   three distinct changes: ${R0:0:12}… → ${R1:0:12}… → ${R2:0:12}… → ${R3:0:12}…" || { echo "  FAIL roots did not change three times"; exit 1; }
    [ "$(field "$T/tx1.txt" old_note_root 2)" = "$R0" ] && [ "$(field "$T/tx2.txt" old_note_root 2)" = "$R1" ] && [ "$(field "$T/tx3.txt" old_note_root 2)" = "$R2" ] \
      && echo "  ok   each proof starts from the root the one before left" || { echo "  FAIL the roots do not chain"; exit 1; }
    i=$N0; for tx in tx1 tx2 tx3; do for col in 2 3; do
      read_eq "$PV" "6c6561663a$(le64 $i)" "$(field "$T/$tx.txt" note_hashes $col)" "leaf $i = $tx's note hash $((col - 1))"; i=$((i + 1)); done; done
    read_eq "$PV" leaves "$(le64 $((N0 + 6)))" "leaves = $((N0 + 6))"
    for _ in 1 2 3 4 5; do [ "$(curl -s "$INDEXER/state" | python3 -c 'import json,sys;print(json.load(sys.stdin).get("note_root",""))')" = "$R3" ] && break; sleep 2; done
    CLIENT_STATE=$SA sync_client >/dev/null; RECOMPUTED=$(CLIENT_STATE=$SA "$CLIENT" root | sed 's/^0x//')
    [ "$RECOMPUTED" = "$R3" ] && echo "  ok   root rebuilt from the chain's $((N0 + 6)) leaves = root:note = ${R3:0:16}…" || { echo "  FAIL rebuilt root $RECOMPUTED != $R3"; exit 1; }

    echo "== nullifiers: exactly these six, and the nullifier root chains through them"
    for tx in tx1 tx2 tx3; do for col in 2 3; do
      read_eq "$PV" "6e663a$(field "$T/$tx.txt" nullifiers $col)" 01 "nf: $tx input $((col - 1))"; done; done
    [ "$(field "$T/tx1.txt" old_null_root 2)" = "$NR0" ] && [ "$(field "$T/tx2.txt" old_null_root 2)" = "$(field "$T/tx1.txt" new_null_root 2)" ] && [ "$(field "$T/tx3.txt" old_null_root 2)" = "$(field "$T/tx2.txt" new_null_root 2)" ] \
      && echo "  ok   root:null chains from before → tx1 → tx2 → tx3: nothing else was spent between them" || { echo "  FAIL the nullifier roots do not chain"; exit 1; }
    read_eq "$PV" root:null "$(field "$T/tx3.txt" new_null_root 2)" "root:null = tx3's"

    echo "== balances and counters"
    read_eq "$PB" "62616c3a$A" "$(le64 $BA0)" "bal:A: +100 minted, -100 shielded"
    read_eq "$PB" "62616c3a$B" "$(le64 $((BB0 + 40)))" "bal:B: +40, by deferred transfer"
    read_eq "$PV" transfers "$(le64 $((T0 + 3)))" "transfers +3"
    read_eq "$PV" shielded "$(le64 $((S0 + 100)))" "shielded +100"
    read_eq "$PV" unshielded "$(le64 $((U0 + 40)))" "unshielded +40"

    echo "== replay the send: a valid proof whose nullifiers are spent"
    echo "  $(submit "$PV" "$(cat "$T/tx2.hex")")"; sleep 9
    read_eq "$PV" root:note "$R3" "root:note unchanged"
    read_eq "$PV" leaves "$(le64 $((N0 + 6)))" "leaves unchanged"
    read_eq "$PV" transfers "$(le64 $((T0 + 3)))" "transfers unchanged"
    read_eq "$PB" "62616c3a$B" "$(le64 $((BB0 + 40)))" "bal:B unchanged"
    echo "== what each client holds"; echo "  A:"; CLIENT_STATE=$SA "$CLIENT" notes; echo "  B:"; CLIENT_STATE=$SB "$CLIENT" notes
    echo "PASS" ;;
  shield|send|unshield)
    # the stages as the Python client: named users A and B (accounts 1 and 2), each with its own
    # note state in out/stage-<name>.json, paid through the directory, finding notes by scanning
    PB=$(sid public-balance); PV=$(sid privacy); T=$ROOT/out; mkdir -p "$T"
    acct() { case "$1" in A) hex32 1 ;; B) hex32 2 ;; *) echo "users: A or B" >&2; exit 2 ;; esac; }
    as() { local u=$1; shift; CLIENT_STATE=$T/stage-$u.json PRIVACY_KEYS=$T/keys.json "$CLIENT" "$@"; }
    publish() { local e; e=$(as "$1" directory "$1")
      [ "$(ctl read "$PB" "6b65793a$(acct "$1")")" = "$e" ] || submit_until "$PB" "03$(acct "$1")$e" "$PB" "6b65793a$(acct "$1")" "$e" "key:$1" >/dev/null; }
    keys() { echo "{\"A\": \"$(ctl read "$PB" "6b65793a$(acct A)")\", \"B\": \"$(ctl read "$PB" "6b65793a$(acct B)")\"}" > "$T/keys.json"; }
    view() { for _ in 1 2 3 4 5 6; do curl -sf "$INDEXER/state" > "$T/stage-chain.json" && break; sleep 1; done; }
    seen() { local root=$1; for _ in $(seq 1 15); do view; python3 -c "import json,sys; sys.exit(0 if json.load(open('$T/stage-chain.json'))['note_root']=='$root' else 1)" && return 0; sleep 2; done; return 1; }
    scan() { view; as "$1" scan "$1" "$T/stage-chain.json" | sed 's/^/  /'; }
    prove_send() { # <user> <a> <b> <to> <value> <unshield> <receiver> <label>
      as "$1" transfer "$1" "$2" "$3" "$4" "$5" "$6" "$7" "$PB" "$T/stage.hex" > "$T/stage.txt"
      submit_until "$PV" "$(cat "$T/stage.hex")" "$PV" root:note "$(field "$T/stage.txt" new_note_root 2)" "$8"; }
    publish A; publish B; keys
    case "$1" in
      shield) U=${2:-A}; N=${3:-100}; scan "$U"
        HAVE=$(u64 "$PB" "62616c3a$(acct "$U")")
        if [ "$HAVE" -lt "$N" ]; then echo "$U holds $HAVE publicly; the faucet mints $((N - HAVE))"
          submit_until "$PB" "00$(acct "$U")$(le64 $((N - HAVE)))$(openssl rand -hex 32)" "$PB" "62616c3a$(acct "$U")" "$(le64 "$N")" "bal:$U"; fi
        echo "$U shields $N"; as "$U" deposit "$U" "$N" > "$T/stage-dep.txt"
        C=$(awk '{print $4}' "$T/stage-dep.txt" | sed 's/^0x//'); D=$(awk '{print $2}' "$T/stage-dep.txt")
        submit_until "$PB" "02$(acct "$U")$(le64 "$N")$(le32 "$PV")$C" "$PV" "6465703a$C" "$(le64 "$N")" "the deposit"
        view; prove_send "$U" "deposit:$D" - "$U" "$N" 0 0 "the claim"
        echo "  $U holds $(as "$U" balance "$U") privately, $(u64 "$PB" "62616c3a$(acct "$U")") publicly" ;;
      send) F=${2:-A}; TO=${3:-B}; N=${4:-40}; scan "$F"; echo "$F sends $N to $TO privately"
        read -r IA IB <<<"$(as "$F" pick "$F" "$N")"; prove_send "$F" "$IA" "$IB" "$TO" "$N" 0 0 "the send"
        echo "  $TO's client scans the chain:"; scan "$TO"
        echo "  $F holds $(as "$F" balance "$F"), $TO holds $(as "$TO" balance "$TO") privately" ;;
      unshield) U=${2:-B}; N=${3:-40}; scan "$U"; echo "$U unshields $N"
        R=$(case "$U" in A) echo 1 ;; B) echo 2 ;; esac)
        read -r IA IB <<<"$(as "$U" pick "$U" "$N")"; prove_send "$U" "$IA" "$IB" "$U" 0 "$N" "$R" "the unshield"
        echo "  $U holds $(as "$U" balance "$U") privately, $(u64 "$PB" "62616c3a$(acct "$U")") publicly" ;;
    esac ;;
  *) sed -n 2,13p "$0"; exit 1 ;;
esac
