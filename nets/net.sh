#!/usr/bin/env bash
# The privacy service on a JAM tiny net, chosen by NET. The native nets are nets/jb.sh's:
# NET=polkajam (the default; PolkaJam's own testnet, no Docker) and the nets jamboree launches
# (NET=jb-pj6, jb-lasair6, jb-mixed). The Docker nets, jamswap's compose, are here:
#   NET=lasair6            six lasair validators, a JIP-2 reader, the CE-133 builder
#   NET=pj6                six PolkaJam validators and an ordinary PolkaJam node for JIP-2
#
#   nets/net.sh up        start the validators and the RPC node (images cached)
#   nets/net.sh build     build both services with jam080-build into out/
#   nets/net.sh deploy    build, then deploy both through the Bootstrap service; ids in out/state/
#   nets/net.sh indexer   (re)start the indexer, HTTP on localhost:19700
#   nets/net.sh ctl ...   `jamboree rpc` inside the net (submit | read | head; nets/ctl.sh)
#   nets/net.sh down      stop the net, the indexer, and wipe the chain
# On lasair6, SUBMIT_VIA_BUILDER=http://builder:19980 (with LASAIR_BUILDER_FANOUT=all at `up`)
# makes the indexer submit through the CE-133 builder instead of the reader (lasair#93).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
JAMSWAP=$SUB/jamswap; BUILD=$JAMSWAP/tools/jam080/builder/target/release/jam080-build
NET=${NET:-polkajam}
case "$NET" in polkajam|jb-*) exec "$HERE/jb.sh" "$@" ;; esac   # the native nets
case "$NET" in
  lasair6) FILE=docker-compose.lasair6.yml; UP="spec-init lm0 lm1 lm2 lm3 lm4 lm5 reader builder"; RPC=ws://reader:19800; RPCSVC=reader ;;
  pj6)     FILE=nets/compose/pj6.yml;       UP="spec-init pj0 pj1 pj2 pj3 pj4 pj5 rpc";              RPC=ws://rpc:42150;    RPCSVC=rpc ;;
  *) echo "NET=$NET: this demo knows polkajam, lasair6, pj6, jb-pj6, jb-lasair6 and jb-mixed" >&2; exit 2 ;;
esac
PROJECT=$NET                                   # jamswap's ./dex uses the net's name too
COMPOSE="docker compose -p $PROJECT -f $JAMSWAP/$FILE"
# the network the RPC node is on and the volume holding spec.json, as compose names them
names() { (cd "$JAMSWAP" && $COMPOSE config --format json) | python3 -c '
import json, sys
c = json.load(sys.stdin); s = c["services"]["'"$RPCSVC"'"]
net = next(iter(s["networks"])); vol = next(v["source"] for v in s["volumes"] if v.get("target") == "/shared")
print(c["networks"][net].get("name", "'"$PROJECT"'_" + net), c["volumes"][vol].get("name", "'"$PROJECT"'_" + vol))'; }
read -r NETWORK VOLUME < <(names)
# `jamboree rpc` (our fork, built for Linux: scripts/bootstrap.sh) in a container on the net:
# the net's JIP-2 endpoint is reachable there and not from the host
JBL=$ROOT/out/jamboree-linux/release/jamboree
jb_linux() { "$ROOT/scripts/jamboree-linux.sh" >/dev/null; }   # built when missing or stale
rpc() { jb_linux >&2; docker run --rm -i --network "$NETWORK" -v "$VOLUME":/shared:ro -v "$ROOT/out":/work \
  -v "$JBL":/usr/local/bin/jamboree:ro python:3.11-slim jamboree rpc --url "$RPC" --chain-spec /shared/spec.json "$@"; }
. "$HERE/ctl.sh"
sid() { python3 -c "import json;print(json.load(open('$ROOT/out/state/$1.json'))['service_id'])"; }
case "${1:-}" in
  up)   (cd "$JAMSWAP" && $COMPOSE up -d $UP) ;;
  down) docker rm -f jam-privacy-indexer >/dev/null 2>&1 || true; (cd "$JAMSWAP" && $COMPOSE down -v --remove-orphans) || true
    # Containers outside the project (an observability stack's collectors) can hold the network
    # past `compose down`; an `up` then creates nodes on a network about to vanish. Detach them
    # and remove it here, so down returns only once the network is gone.
    if docker network inspect "$NETWORK" >/dev/null 2>&1; then
      for c in $(docker network inspect "$NETWORK" --format '{{range .Containers}}{{.Name}} {{end}}'); do
        docker network disconnect -f "$NETWORK" "$c" >/dev/null 2>&1 || true
      done
      docker network rm "$NETWORK" >/dev/null 2>&1 || true
    fi
    ! docker network inspect "$NETWORK" >/dev/null 2>&1 || { echo "network $NETWORK is still there" >&2; exit 1; } ;;
  build) mkdir -p "$ROOT/out"; "$BUILD" "$ROOT/services/public-balance" --out "$ROOT/out"; "$BUILD" "$ROOT/services/privacy" --out "$ROOT/out" ;;
  deploy)
    "$0" build; mkdir -p "$ROOT/out/state"
    for svc in public-balance privacy; do rpc deploy "/work/$svc-service.jam" --state "/work/state/$svc.json" | tail -1; done ;;
  ctl) shift; ctl "$@" ;;
  indexer)
    docker rm -f jam-privacy-indexer >/dev/null 2>&1 || true
    jb_linux
    docker run -d --name jam-privacy-indexer --network "$NETWORK" -p 127.0.0.1:19700:19700 \
      -v "$VOLUME:/shared:ro" -v "$ROOT/indexer:/indexer:ro" -v "$JBL":/usr/local/bin/jamboree:ro \
      -e CHAIN_RPC="$RPC" -e CHAIN_SPEC=/shared/spec.json -e PRIVACY_ID="$(sid privacy)" -e PUBLIC_ID="$(sid public-balance)" \
      -e NET_NAME="$NET" -e SUBMIT_VIA_BUILDER="${SUBMIT_VIA_BUILDER:-}" python:3.11-slim python3 /indexer/indexer.py >/dev/null
    for _ in $(seq 1 30); do curl -sf localhost:19700/state >/dev/null 2>&1 && break; sleep 1; done
    curl -s localhost:19700/state | python3 -c "import json,sys; d=json.load(sys.stdin); print('indexer up on $NET: slot', d.get('slot'), 'leaves', d.get('leaf_count'), 'transfers', d.get('transfers'))" ;;
  names) echo "$NET: project $PROJECT, network $NETWORK, volume $VOLUME, rpc $RPC" ;;
  *) sed -n 2,12p "$0"; exit 1 ;;
esac
