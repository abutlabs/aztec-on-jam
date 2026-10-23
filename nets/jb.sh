#!/usr/bin/env bash
# The demo on native nets that jamboree (our fork, ../jamboree, branch `lasair`) launches,
# with nothing of jamswap: jamboree runs the nodes, `jamboree rpc` deploys and submits, the
# indexer reads with its own JIP-2 client. nets/net.sh hands NET=jb-* here.
#   NET=jb-pj6       six PolkaJam validators, and an ordinary PolkaJam node serving JIP-2
#   NET=jb-lasair6   six lasair validators, and lasair-reader serving JIP-2
#   NET=jb-mixed     PolkaJam validators 0-2, lasair validators 3-5, the PolkaJam node's JIP-2
# Every net starts from PolkaJam's dev spec (`polkajam --chain dev dump-spec`): validator i is
# JAM dev account i, at 127.0.0.1:40000+i (jamboree's genesis ports), the Bootstrap service is
# service 0. lasair dials its peers by those addresses and the dev accounts' peer ids.
#
#   nets/jb.sh up | build | deploy | indexer | ctl ... | check | names | down
# Env: POLKAJAM, LASAIR (the client binaries), JAMBOREE, JB_MINUTES (how long jamboree keeps
#      the net up, headless; default 240), JB_TELEMETRY (PolkaJam's JIP-3 telemetry to host:port)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
NET=${NET:-jb-pj6}
JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}
# PolkaJam v0.8.0 (scripts/bootstrap.sh fetches it): its PVM gas matches lasair main's (the
# v0.8.0 vectors' cmov rule); the 2026-09-22 nightly charges 2 gas less on some accumulates,
# and on a mixed net lasair then rejects PolkaJam's blocks (bad_parent_state_root, key 0x0d)
PJ=${POLKAJAM:-$(ls "$HOME"/.cache/jam-privacy/polkajam-v0.8.0/polkajam-v0.8.0-*/polkajam 2>/dev/null | head -1)}
LZ=${LASAIR:-$SUB/lasair/_build/default/bin/lasair_client.exe}
LR=${LASAIR_READER:-$SUB/lasair/_build/default/bin/lasair_reader.exe}
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build   # the GP 0.8.0 blob builder
RUN=$ROOT/out/$NET; SPEC=$RUN/spec.json; RPC_PORT=42950; RPC=ws://127.0.0.1:$RPC_PORT
GENESIS_PORT=40000; BUILDER_BASE=42100                               # lasair i's CE-133 port: 42100+i
rpc() { "$JB" rpc --url "$RPC" --chain-spec "$SPEC" "$@"; }
sid() { python3 -c "import json;print(json.load(open('$ROOT/out/state/$1.json'))['service_id'])"; }
case "$NET" in
  jb-pj6)     PJN=6; LZN=0 ;;
  jb-lasair6) PJN=0; LZN=6 ;;
  jb-mixed)   PJN=3; LZN=3 ;;
  *) echo "NET=$NET: jb.sh knows jb-pj6, jb-lasair6, jb-mixed" >&2; exit 2 ;;
esac

# lasair validator i, as jamboree runs it: listening on the port jamboree moved it to,
# dialing every other validator at its genesis port with the dev account's peer id
lasair_node() {
  local i=$1 port=$2 peers="" k pid
  for k in 0 1 2 3 4 5; do
    [ "$k" = "$i" ] && continue
    pid=$("$LZ" --dev-account "$k" | awk '/^peer_id:/{print $2}')
    peers="$peers,127.0.0.1:$((GENESIS_PORT + k))@$pid"
  done
  # pacing as jamswap's lasair6 and mixed nets: with PolkaJam on the net, wall-clock slots;
  # alone, chain-paced from genesis slot 0, a slot every 6 s (wall-paced, a head at slot 0
  # "lags" today's wall slot by millions and the node back-fills forever instead of authoring)
  local pace=(--interval 6)
  [ "$PJN" -gt 0 ] && pace=(--interval 1 --wall)
  exec "$LZ" --dev-validator "$i" --genesis-spec "$SPEC" --port "$port" --peers "${peers#,}" \
    --guarantor --builder-port $((BUILDER_BASE + i)) --metrics-port 0 "${pace[@]}" \
    --spec-out "$RUN/spec-out-$i.json"
}

# the JIP-2 endpoint: an ordinary PolkaJam node (a validator's RPC does not forward
# packages), or lasair-reader following lasair validator 3 (or 0) and submitting to every
# lasair validator's CE-133 port
rpc_node() {
  if [ "$PJN" -gt 0 ]; then
    exec "$PJ" --chain "$SPEC" run --temp --pvm-backend interpreter --mode ordinary --port "$1" --rpc-port "$RPC_PORT"
  fi
  local g="" i
  for i in 0 1 2 3 4 5; do g="$g,127.0.0.1:$((BUILDER_BASE + i))"; done
  LASAIR_NODE_HOST=127.0.0.1 LASAIR_NODE_PORT=$GENESIS_PORT LASAIR_CHAIN_SPEC="$SPEC" \
    LASAIR_READER_HTTP_PORT=$RPC_PORT LASAIR_RPC_GUARANTORS="${g#,}" exec "$LR"
}

case "${1:-}" in
  up)
    [ -x "$JB" ] || (cd "$SUB/jamboree" && cargo build --release -q)
    [ "$PJN" = 0 ] || [ -x "$PJ" ] || { echo "no PolkaJam binary at $PJ (scripts/bootstrap.sh fetches it)" >&2; exit 1; }
    [ "$LZN" = 0 ] || [ -x "$LZ" ] || { echo "no lasair binary at $LZ (dune build in ../lasair)" >&2; exit 1; }
    rm -rf "$RUN"; mkdir -p "$RUN"
    "$PJ" --chain dev dump-spec "$SPEC" >/dev/null
    cmds=()
    # the interpreter: PolkaJam's recompiler is Linux-only. JB_TELEMETRY=host:port sends its JIP-3
    # telemetry there (e.g. PolkaJam's jam-telemetry-dumper, for refine times)
    [ "$PJN" -gt 0 ] && cmds+=(--cmd "polkajam[$PJN]=$PJ --chain $SPEC run --temp --pvm-backend interpreter --dev-validator {i} --port {port} --rpc-port \$((42900+{i}))${JB_TELEMETRY:+ --telemetry $JB_TELEMETRY}")
    [ "$LZN" -gt 0 ] && cmds+=(--cmd "lasair[$LZN]=NET=$NET $HERE/jb.sh lasair-node {i} {port}")
    cmds+=(--cmd "rpc=NET=$NET $HERE/jb.sh rpc-node {port}")
    ( cd "$RUN" && nohup "$JB" --headless "${JB_MINUTES:-240}m" --move-ports "${cmds[@]}" \
        > "$RUN/jamboree.log" 2>&1 & echo $! > "$RUN/jamboree.pid" )
    # ready once finality has left genesis: the dev genesis is at slot 0 and the first block
    # at today's wall-clock slot, and a package anchored before the nodes agree is refused
    # ("invalid epoch …, reference epoch is 0")
    for _ in $(seq 1 120); do rpc head 2>/dev/null | grep -qv "finalized 0 " && break; sleep 2; done
    rpc head ;;
  lasair-node) lasair_node "$2" "$3" ;;
  rpc-node) rpc_node "$2" ;;
  down)
    for p in indexer wallet; do [ -f "$RUN/$p.pid" ] && kill "$(cat "$RUN/$p.pid")" 2>/dev/null || true; done
    [ -f "$RUN/jamboree.pid" ] && kill "$(cat "$RUN/jamboree.pid")" 2>/dev/null || true
    pkill -f -- "--chain $SPEC" 2>/dev/null || true; pkill -f -- "--genesis-spec $SPEC" 2>/dev/null || true
    pkill -f -- "jb.sh rpc-node" 2>/dev/null || true; pkill -f "$LR" 2>/dev/null || true
    sleep 2; rm -rf "$RUN" ;;
  build) mkdir -p "$ROOT/out"; "$BUILD" "$ROOT/services/public-balance" --out "$ROOT/out"; "$BUILD" "$ROOT/services/privacy" --out "$ROOT/out" ;;
  deploy)
    "$0" build; mkdir -p "$ROOT/out/state"
    for svc in public-balance privacy; do
      rpc deploy "$ROOT/out/$svc-service.jam" --state "$ROOT/out/state/$svc.json" | tail -1
    done ;;
  indexer)
    [ -f "$RUN/indexer.pid" ] && kill "$(cat "$RUN/indexer.pid")" 2>/dev/null || true
    docker rm -f jam-privacy-indexer >/dev/null 2>&1 || true      # a Docker net's, if any
    JAMBOREE=$JB CHAIN_RPC=$RPC CHAIN_SPEC=$SPEC PRIVACY_ID=$(sid privacy) PUBLIC_ID=$(sid public-balance) \
      NET_NAME=$NET nohup python3 "$ROOT/indexer/indexer.py" > "$RUN/indexer.log" 2>&1 & echo $! > "$RUN/indexer.pid"
    for _ in $(seq 1 30); do curl -sf localhost:19700/state >/dev/null 2>&1 && break; sleep 1; done
    curl -s localhost:19700/state | python3 -c "import json,sys; d=json.load(sys.stdin); print('indexer up on $NET: slot', d.get('slot'), 'leaves', d.get('leaf_count'), 'transfers', d.get('transfers'))" ;;
  ctl)  # as nets/jamctl.py: submit <sid> <payload-hex> | read <sid> <key> | head
    shift
    case "$1" in
      head) rpc head ;;
      read) k=$3; [[ "$k" =~ ^[0-9a-fA-F]+$ && ${#k} -gt 8 && $(( ${#k} % 2 )) -eq 0 ]] && k=0x$k; rpc read "$2" "$k" ;;
      submit) out=$(rpc submit "$2" "0x$3" --wait --timeout "${WAIT:-240}" 2>&1) || true
        echo "$out" | head -1 | sed 's/^/submitted /'
        echo "$out" | sed -n 's/^{"status":\(.*\)}$/status \1/p' ;;
      *) echo "ctl: submit | read | head" >&2; exit 2 ;;
    esac ;;
  names) echo "$NET: jamboree $JB, PolkaJam $PJ ($PJN), lasair $LZ ($LZN), rpc $RPC, spec $SPEC" ;;
  *) sed -n 2,14p "$0"; exit 1 ;;
esac
