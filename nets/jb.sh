#!/usr/bin/env bash
# The demo on a native net that jamboree (our fork, ../jamboree, branch `lasair`) launches,
# with nothing of jamswap: jamboree runs the nodes, `jamboree rpc` deploys and submits, the
# indexer reads with its own JIP-2 client. nets/net.sh hands NET=jb-* here.
#   NET=jb-pj6     six PolkaJam validators and an ordinary PolkaJam node serving JIP-2
#
#   nets/jb.sh up | build | deploy | indexer | ctl ... | names | down
# Env: POLKAJAM (the binary; default the pinned nightly under ~/.cache/jam-privacy),
#      JB_MINUTES (how long jamboree keeps the net up, headless; default 240)
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
NET=${NET:-jb-pj6}
JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}
PJ=${POLKAJAM:-$HOME/.cache/jam-privacy/polkajam-nightly-2026-09-22/polkajam-nightly-2026-09-22-macos-aarch64/polkajam}
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build   # jam080-build: the GP 0.8.0 blob builder
RUN=$ROOT/out/$NET; SPEC=$RUN/spec.json; RPC_PORT=42950; RPC=ws://127.0.0.1:$RPC_PORT
rpc() { "$JB" rpc --url "$RPC" --chain-spec "$SPEC" "$@"; }
sid() { python3 -c "import json;print(json.load(open('$ROOT/out/state/$1.json'))['service_id'])"; }
case "$NET" in jb-pj6) ;; *) echo "NET=$NET: jb.sh knows jb-pj6" >&2; exit 2 ;; esac

case "${1:-}" in
  up)
    [ -x "$JB" ] || (cd "$SUB/jamboree" && cargo build --release -q)
    [ -x "$PJ" ] || { echo "no PolkaJam binary at $PJ (scripts/bootstrap.sh fetches it)" >&2; exit 1; }
    rm -rf "$RUN"; mkdir -p "$RUN"
    "$PJ" --chain dev dump-spec "$SPEC" >/dev/null
    # six validators (jamboree relays the genesis ports and moves each node to {port}), and an
    # ordinary node whose RPC takes submissions (a validator's RPC does not forward packages)
    ( cd "$RUN" && nohup "$JB" --headless "${JB_MINUTES:-240}m" --move-ports \
        --cmd "pj[6]=$PJ --chain $SPEC run --temp --pvm-backend interpreter --dev-validator {i} --port {port} --rpc-port \$((42900+{i}))" \
        --cmd "rpc=$PJ --chain $SPEC run --temp --pvm-backend interpreter --mode ordinary --port {port} --rpc-port $RPC_PORT" \
        > "$RUN/jamboree.log" 2>&1 & echo $! > "$RUN/jamboree.pid" )
    for _ in $(seq 1 90); do rpc head >/dev/null 2>&1 && break; sleep 2; done
    rpc head ;;
  down)
    for p in indexer wallet; do [ -f "$RUN/$p.pid" ] && kill "$(cat "$RUN/$p.pid")" 2>/dev/null || true; done
    [ -f "$RUN/jamboree.pid" ] && kill "$(cat "$RUN/jamboree.pid")" 2>/dev/null || true
    pkill -f -- "--chain $SPEC" 2>/dev/null || true; sleep 2; rm -rf "$RUN" ;;
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
  names) echo "$NET: jamboree $JB, PolkaJam $PJ, rpc $RPC, spec $SPEC" ;;
  *) sed -n 2,11p "$0"; exit 1 ;;
esac
