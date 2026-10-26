#!/usr/bin/env bash
# The demo on native nets, with no Docker and nothing of jamswap's: the nodes run on this
# machine, `jamboree rpc` (our fork, ../jamboree, branch `lasair`) deploys and submits, the
# indexer reads with its own JIP-2 client. nets/net.sh hands NET=polkajam and NET=jb-* here.
#   NET=polkajam     (the default) PolkaJam's own tiny testnet: the release's `polkajam-testnet`
#                    starts six validators and an ordinary node serving JIP-2, GRANDPA finality
# The jb-* nets are launched by jamboree, which can mix clients:
#   NET=jb-pj6       six PolkaJam validators, and an ordinary PolkaJam node serving JIP-2
#   NET=jb-lasair6   six lasair validators, and lasair-reader serving JIP-2
#   NET=jb-mixed     PolkaJam validators 0-2, lasair validators 3-5, the PolkaJam node's JIP-2
# Every net starts from PolkaJam's dev spec (`polkajam --chain dev dump-spec`): validator i is
# JAM dev account i, at 127.0.0.1:40000+i (jamboree's genesis ports), the Bootstrap service is
# service 0. lasair dials its peers by those addresses and the dev accounts' peer ids.
#
#   nets/jb.sh up | build | deploy | indexer | ctl ... | names | down | headcheck [minutes]
# Env: POLKAJAM, LASAIR (the client binaries), JAMBOREE, JB_MINUTES (how long jamboree keeps
#      the net up, headless; default 240), JB_TELEMETRY (the nodes' JIP-3 telemetry to host:port,
#      e.g. 127.0.0.1:9910 for the observability stack's receiver), JB_RPC (on an all-lasair
#      net: node, the default, for JIP-2 from validator 0's own state; reader for lasair-reader),
#      LASAIR_JIP8_FROM_SLOT (lasair's JIP-8 curve host calls from that slot, on every node: an
#      experiment, not GP 0.8.0; unset, off)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
NET=${NET:-polkajam}
JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}
# PolkaJam v0.8.0 (scripts/bootstrap.sh fetches it): its PVM gas matches lasair main's (the
# v0.8.0 vectors' cmov rule); the 2026-09-22 nightly charges 2 gas less on some accumulates,
# and on a mixed net lasair then rejects PolkaJam's blocks (bad_parent_state_root, key 0x0d)
case "$(uname -s)-$(uname -m)" in   # the release build for this machine (bootstrap fetches it)
  Darwin-arm64) PJP=macos-aarch64 ;; Darwin-x86_64) PJP=macos-x86_64 ;;
  Linux-aarch64|Linux-arm64) PJP=linux-aarch64 ;; *) PJP=linux-x86_64 ;;
esac
PJ=${POLKAJAM:-$HOME/.cache/jam-privacy/polkajam-v0.8.0/polkajam-v0.8.0-$PJP/polkajam}
PJT=$(dirname "$PJ")/polkajam-testnet                                 # the same release's launcher
LZ=${LASAIR:-$SUB/lasair/_build/default/bin/lasair_client.exe}
LR=${LASAIR_READER:-$SUB/lasair/_build/default/bin/lasair_reader.exe}
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build   # the GP 0.8.0 blob builder
RUN=${RUN:-$ROOT/out/$NET}; SPEC=${SPEC:-$RUN/spec.json}; RPC_PORT=42950; RPC=ws://127.0.0.1:$RPC_PORT
GENESIS_PORT=40000; BUILDER_BASE=42100                               # lasair i's CE-133 port: 42100+i
rpc() { "$JB" rpc --url "$RPC" --chain-spec "$SPEC" "$@"; }
. "$HERE/ctl.sh"
sid() { python3 -c "import json;print(json.load(open('$ROOT/out/state/$1.json'))['service_id'])"; }
CHAIN=dev                     # PolkaJam's dev chain: validator i at 127.0.0.1:GENESIS_PORT+i
case "$NET" in
  # its own genesis ports, off the 40000 that the jb-* nets and spikes/lib/polkajam.sh use, so
  # a spike's testnet and this one never take each other's traffic; the dev chain built for
  # them (dev:41000) records those addresses in its genesis
  polkajam)   PJN=6; LZN=0; GENESIS_PORT=41000; CHAIN=dev:$GENESIS_PORT ;;
  jb-pj6)     PJN=6; LZN=0 ;;
  jb-lasair6) PJN=0; LZN=6 ;;
  jb-mixed)   PJN=3; LZN=3 ;;
  *) echo "NET=$NET: jb.sh knows polkajam, jb-pj6, jb-lasair6, jb-mixed" >&2; exit 2 ;;
esac

# another native net (or anything) on the genesis ports would take this net's nodes' traffic
ports_free() {
  local held; held=$( { lsof -nP -iUDP:$GENESIS_PORT -iUDP:$((GENESIS_PORT + 5)) 2>/dev/null || true; } | awk 'NR>1{print $1" "$2}' | sort -u | head -3)
  [ -z "$held" ] || { echo "the genesis ports $GENESIS_PORT.. are in use (another net is up?): $held" >&2; exit 2; }
}

# lasair validator i, as jamboree runs it: listening on the port jamboree moved it to,
# dialing every other validator at its genesis port with the dev account's peer id
# JIP-2 from a lasair validator itself (JB_RPC=node, the default on an all-lasair net), or from
# lasair-reader following one (JB_RPC=reader: a block behind, its reads proven)
node_rpc() { [ "$PJN" = 0 ] && [ "${JB_RPC:-node}" = node ]; }

lasair_node() {
  local i=$1 port=$2 tel=${3:-} peers="" k pid
  for k in 0 1 2 3 4 5; do
    [ "$k" = "$i" ] && continue
    pid=$("$LZ" --dev-account "$k" | awk '/^peer_id:/{print $2}')
    peers="$peers,127.0.0.1:$((GENESIS_PORT + k))@$pid"
  done
  # pacing as jamswap's lasair6 and mixed nets: with PolkaJam on the net, wall-clock slots;
  # alone, chain-paced from genesis slot 0 (wall-paced, a head at slot 0 "lags" today's wall
  # slot by millions and the node back-fills forever instead of authoring), a block no sooner
  # than 6 s after the last, polled every second: JAM's slot time (with --interval 6 alone the
  # slot owner authored whenever its poll fell, a block about every 3.9 s)
  local pace=(--interval 1 --block-time 6)
  [ "$PJN" -gt 0 ] && pace=(--interval 1 --wall)
  # on an all-lasair net, validator 0 serves JIP-2 itself from its own state (lasair#83):
  # the head, not a block behind; work-packages go over CE-133 to every lasair builder port
  local rpc=() g="" k2
  if node_rpc && [ "$i" = 0 ]; then
    for k2 in 0 1 2 3 4 5; do g="$g,127.0.0.1:$((BUILDER_BASE + k2))"; done
    rpc=(--rpc-port "$RPC_PORT"); export LASAIR_RPC_GUARANTORS="${g#,}"
  fi
  exec "$LZ" --dev-validator "$i" --genesis-spec "$SPEC" --port "$port" --peers "${peers#,}" \
    --guarantor --builder-port $((BUILDER_BASE + i)) --metrics-port 0 "${pace[@]}" "${rpc[@]}" \
    --spec-out "$RUN/spec-out-$i.json" ${tel:+--telemetry "$tel"}
}

# the JIP-2 endpoint: an ordinary PolkaJam node (a validator's RPC does not forward
# packages), or lasair-reader following lasair validator 3 (or 0) and submitting to every
# lasair validator's CE-133 port
rpc_node() {
  if [ "$PJN" -gt 0 ]; then
    exec "$PJ" --chain "$SPEC" run --temp --pvm-backend interpreter --finality-mode grandpa --mode ordinary --port "$1" --rpc-port "$RPC_PORT"
  fi
  local g="" i
  for i in 0 1 2 3 4 5; do g="$g,127.0.0.1:$((BUILDER_BASE + i))"; done
  LASAIR_NODE_HOST=127.0.0.1 LASAIR_NODE_PORT=$GENESIS_PORT LASAIR_CHAIN_SPEC="$SPEC" \
    LASAIR_READER_HTTP_PORT=$RPC_PORT LASAIR_RPC_GUARANTORS="${g#,}" exec "$LR"
}

# PolkaJam's own testnet: validators at the genesis ports, its ordinary node's JIP-2 on RPC_PORT.
# Its working directory (keys, chain data) goes under $RUN through TMPDIR, so `down` wipes it.
# The launcher does not stop its nodes when it is stopped: `down` stops its children first.
testnet_up() {
  [ -x "$PJT" ] || { echo "no polkajam-testnet at $PJT (scripts/bootstrap.sh fetches PolkaJam v0.8.0)" >&2; exit 1; }
  # one simple command in the background, so $! is the launcher itself and nothing holds our stdout
  ( cd "$RUN" || exit 1
    TMPDIR=$RUN nohup "$PJT" --base-port $GENESIS_PORT --base-rpc-port $RPC_PORT --finality-mode grandpa \
      ${JB_TELEMETRY:+--telemetry $JB_TELEMETRY} > "$RUN/testnet.log" 2>&1 < /dev/null & echo $! > "$RUN/testnet.pid" )
}
testnet_down() {
  local pid; [ -f "$RUN/testnet.pid" ] && pid=$(cat "$RUN/testnet.pid") || return 0
  pkill -P "$pid" 2>/dev/null || true; kill "$pid" 2>/dev/null || true
  pkill -f -- "--config-path $RUN/" 2>/dev/null || true     # a node the launcher left behind
}

case "${1:-}" in
  up)
    ports_free
    [ -x "$JB" ] || (cd "$SUB/jamboree" && cargo build --release -q)
    [ "$PJN" = 0 ] || [ -x "$PJ" ] || { echo "no PolkaJam binary at $PJ (scripts/bootstrap.sh fetches it)" >&2; exit 1; }
    [ "$LZN" = 0 ] || [ -x "$LZ" ] || { echo "no lasair binary at $LZ (dune build in ../lasair)" >&2; exit 1; }
    rm -rf "$RUN"; mkdir -p "$RUN"
    "$PJ" --chain "$CHAIN" dump-spec "$SPEC" >/dev/null
    if [ "$NET" = polkajam ]; then testnet_up; else
    cmds=()
    # the interpreter: PolkaJam's recompiler is Linux-only. JB_TELEMETRY=host:port sends its JIP-3
    # telemetry there (e.g. PolkaJam's jam-telemetry-dumper, for refine times)
    [ "$PJN" -gt 0 ] && cmds+=(--cmd "polkajam[$PJN]=$PJ --chain $SPEC run --temp --pvm-backend interpreter --finality-mode grandpa --dev-validator {i} --port {port} --rpc-port \$((42900+{i}))${JB_TELEMETRY:+ --telemetry $JB_TELEMETRY}")
    [ "$LZN" -gt 0 ] && cmds+=(--cmd "lasair[$LZN]=NET=$NET $HERE/jb.sh lasair-node {i} {port}${JB_TELEMETRY:+ $JB_TELEMETRY}")
    node_rpc || cmds+=(--cmd "rpc=NET=$NET $HERE/jb.sh rpc-node {port}")
    ( cd "$RUN" || exit 1
      nohup "$JB" --headless "${JB_MINUTES:-240}m" --move-ports "${cmds[@]}" \
        > "$RUN/jamboree.log" 2>&1 < /dev/null & echo $! > "$RUN/jamboree.pid" )
    fi
    # ready once finality has left genesis: the dev genesis is at slot 0 and the first block
    # at today's wall-clock slot, and a package anchored before the nodes agree is refused
    # ("invalid epoch …, reference epoch is 0")
    for _ in $(seq 1 120); do rpc head 2>/dev/null | grep -qv "finalized 0 " && break; sleep 2; done
    rpc head ;;
  lasair-node) lasair_node "$2" "$3" "${4:-}" ;;
  headcheck)
    # jamboree's own check: the validators alone, headless for N minutes (default 3), every node
    # sending JIP-3 telemetry to jamboree; exit 0 only if all six end on the same finalized block.
    # Uses the genesis ports, so the net must be down.
    [ -f "$RUN/jamboree.pid" ] && kill -0 "$(cat "$RUN/jamboree.pid")" 2>/dev/null && { echo "$NET is up: nets/jb.sh down first" >&2; exit 2; }
    ports_free
    [ -x "$JB" ] || (cd "$SUB/jamboree" && cargo build --release -q)
    RUN=$ROOT/out/$NET-headcheck; SPEC=$RUN/spec.json; rm -rf "$RUN"; mkdir -p "$RUN"
    "$PJ" --chain dev dump-spec "$SPEC" >/dev/null
    cmds=()
    [ "$PJN" -gt 0 ] && cmds+=(--cmd "polkajam[$PJN]=$PJ --chain $SPEC run --temp --pvm-backend interpreter --finality-mode grandpa --dev-validator {i} --port {port} --telemetry {telemetry}")
    [ "$LZN" -gt 0 ] && cmds+=(--cmd "lasair[$LZN]=NET=$NET RUN=$RUN SPEC=$SPEC $HERE/jb.sh lasair-node {i} {port} {telemetry}")
    rc=0; ( cd "$RUN" && "$JB" --headless "${2:-3}m" --move-ports --head jip3-final "${cmds[@]}" > "$RUN/jamboree.log" 2>&1 ) || rc=$?
    tail -8 "$RUN/jamboree.log"; echo "headcheck $NET: jamboree exit $rc"; exit $rc ;;
  rpc-node) rpc_node "$2" ;;
  down)
    for p in indexer wallet; do [ -f "$RUN/$p.pid" ] && kill "$(cat "$RUN/$p.pid")" 2>/dev/null || true; done
    [ -f "$RUN/jamboree.pid" ] && kill "$(cat "$RUN/jamboree.pid")" 2>/dev/null || true
    testnet_down
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
  ctl) shift; ctl "$@" ;;
  names) echo "$NET: jamboree $JB, PolkaJam $PJ ($PJN), lasair $LZ ($LZN), rpc $RPC, spec $SPEC" ;;
  *) sed -n 2,14p "$0"; exit 1 ;;
esac
