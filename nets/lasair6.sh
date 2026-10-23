#!/usr/bin/env bash
# The privacy service on jamswap's lasair6 net (six lasair validators, JIP-2 on `reader`).
#   nets/lasair6.sh up        start the validators, a reader and the builder (images cached)
#   nets/lasair6.sh deploy    build both services with jam080-build and deploy them through
#                             the Bootstrap service; ids land in out/state/*.json
#   nets/lasair6.sh ctl ...   nets/jamctl.py inside the net (submit | read | head)
#   nets/lasair6.sh indexer   (re)start the indexer container, HTTP on localhost:19700
#   nets/lasair6.sh down      stop the net and wipe the chain
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
JAMSWAP=$SUB/jamswap; COMPOSE="docker compose -f $JAMSWAP/docker-compose.lasair6.yml"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
PY="docker run --rm -i --network jamswap_lasnet -v $JAMSWAP/offchain:/offchain:ro -v jamswap_lasair6_shared:/shared:ro -v $ROOT/out:/work -v $HERE/jamctl.py:/jamctl.py:ro python:3.11-slim"
case "${1:-}" in
  up)   (cd "$JAMSWAP" && $COMPOSE up -d spec-init lm0 lm1 lm2 lm3 lm4 lm5 reader builder) ;;
  down) docker rm -f jam-privacy-indexer >/dev/null 2>&1 || true; (cd "$JAMSWAP" && $COMPOSE down -v) ;;
  build) mkdir -p "$ROOT/out"; "$BUILD" "$ROOT/services/public-balance" --out "$ROOT/out"; "$BUILD" "$ROOT/services/privacy" --out "$ROOT/out" ;;
  deploy)
    "$0" build; mkdir -p "$ROOT/out/state"
    for svc in public-balance privacy; do
      $PY python3 /offchain/deploy.py --rpc ws://reader:19800 --chain-spec /shared/spec.json \
        --code /work/$svc-service.jam --state /work/state/$svc.json --no-setup --timeout 900 | tail -1
    done ;;
  ctl) shift; $PY python3 /jamctl.py "$@" ;;
  indexer)
    docker rm -f jam-privacy-indexer >/dev/null 2>&1 || true
    PV=$(python3 -c "import json;print(json.load(open('$ROOT/out/state/privacy.json'))['service_id'])")
    PB=$(python3 -c "import json;print(json.load(open('$ROOT/out/state/public-balance.json'))['service_id'])")
    docker run -d --name jam-privacy-indexer --network jamswap_lasnet -p 127.0.0.1:19700:19700 \
      -v "$JAMSWAP/offchain:/offchain:ro" -v jamswap_lasair6_shared:/shared:ro -v "$ROOT/indexer/indexer.py:/indexer.py:ro" \
      -e PRIVACY_ID="$PV" -e PUBLIC_ID="$PB" python:3.11-slim python3 /indexer.py >/dev/null
    for _ in $(seq 1 30); do curl -sf localhost:19700/state >/dev/null 2>&1 && break; sleep 1; done
    curl -s localhost:19700/state | python3 -c "import json,sys; d=json.load(sys.stdin); print('indexer up: slot', d.get('slot'), 'leaves', d.get('leaf_count'), 'transfers', d.get('transfers'))" ;;
  *) sed -n 2,8p "$0"; exit 1 ;;
esac
