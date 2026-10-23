#!/usr/bin/env bash
# The demo, one word a stage. Runs against jamswap's lasair6 net (nets/lasair6.sh).
#   nets/demo.sh up        start the net, build and deploy both services
#   nets/demo.sh check     shield 100 from A, send (the stub proof), unshield 40 to B; assert
#                          the chain's state byte for byte; a replay must change nothing
#   nets/demo.sh down      stop the net and wipe the chain
# Accounts are 32-byte big-endian field elements: A = 1, B = 2. Service ids come from
# out/state/*.json (deploy.py's state files).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd)
L=$HERE/lasair6.sh
sid() { python3 -c "import json;print(json.load(open('$ROOT/out/state/$1.json'))['service_id'])"; }
hex32() { printf '%064x' "$1"; }                 # a small number as a 32-byte BE field
le64() { python3 -c "import struct;print(struct.pack('<Q',$1).hex())"; }
le32() { python3 -c "import struct;print(struct.pack('<I',$1).hex())"; }
ctl() { "$L" ctl "$@" 2>&1 | tail -1; }
submit() { WAIT=${WAIT:-300} "$L" ctl submit "$@" 2>&1 | grep -E "^status .*(Ready|Failed)|timeout" | tail -1 | cut -c1-60; }
read_eq() { # <sid> <key-hex|text> <expected-hex|"(none)"> <name>; retries while the block settles
  local got; for _ in 1 2 3 4 5 6 7 8 9 10; do got=$(ctl read "$1" "$2"); [ "$got" = "$3" ] && break; sleep 3; done
  if [ "$got" = "$3" ]; then echo "  ok   $4 = $got"; else echo "  FAIL $4: wanted $3, got $got"; exit 1; fi
}
case "${1:-}" in
  up) "$L" up; sleep 30; "$L" deploy ;;
  down) "$L" down ;;
  check)
    PB=$(sid public-balance); PV=$(sid privacy); echo "public-balance = service $PB, privacy = service $PV"
    A=$(hex32 1); B=$(hex32 2); C=$(hex32 501)      # C: the first note hash the stub proof carries
    F=$ROOT/services/privacy/fixtures
    PUBS=$(xxd -p -c 1000 "$F/sample.pubs" | tr -d '\n'); PROOF=$(xxd -p -c 100000 "$F/sample.proof" | tr -d '\n')
    LEN=$(( ${#PUBS} / 2 )); PAYLOAD="$(printf '%02x%02x' $((LEN & 255)) $((LEN >> 8)))$PUBS$PROOF"
    echo "== mint 100 to A";            echo "  $(submit "$PB" "00$A$(le64 100)")"
    read_eq "$PB" "62616c3a$A" "$(le64 100)" "bal:A"
    echo "== shield 100 from A into the privacy service, commitment C"
    echo "  $(submit "$PB" "02$A$(le64 100)$(le32 "$PV")$C")"
    read_eq "$PB" "62616c3a$A" "$(le64 0)" "bal:A"
    read_eq "$PV" "6465703a$C" "$(le64 100)" "dep:C (the deposit arrived by deferred transfer)"
    read_eq "$PV" "shielded" "$(le64 100)" "shielded"
    echo "== send: the stub transfer proof, which also unshields 40 to B"
    echo "  $(submit "$PV" "$PAYLOAD")"
    read_eq "$PV" "root:note" "$(hex32 1001)" "root:note"
    read_eq "$PV" "root:null" "$(hex32 2002)" "root:null"
    read_eq "$PV" "6e663a1e95b928248aa2a64eeb6e05d80a5742a6d73691cb2666c19d3bcc8dc0a429d3" "01" "nf:<nullifier 0>"
    read_eq "$PV" "6e663a$(hex32 42)" "01" "nf:<nullifier 1>"
    read_eq "$PV" "transfers" "$(le64 1)" "transfers"
    read_eq "$PV" "unshielded" "$(le64 40)" "unshielded"
    read_eq "$PB" "62616c3a$B" "$(le64 40)" "bal:B (the unshield arrived by deferred transfer)"
    echo "== replay the same proof: a valid proof, a spent nullifier"
    echo "  $(submit "$PV" "$PAYLOAD")"
    sleep 8
    read_eq "$PV" "transfers" "$(le64 1)" "transfers unchanged"
    read_eq "$PV" "unshielded" "$(le64 40)" "unshielded unchanged"
    read_eq "$PB" "62616c3a$B" "$(le64 40)" "bal:B unchanged"
    echo "PASS" ;;
  *) sed -n 2,8p "$0"; exit 1 ;;
esac
