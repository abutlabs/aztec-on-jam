#!/usr/bin/env bash
# The privacy service on jamswap's lasair6 net (six lasair validators, JIP-2 on `reader`).
#   nets/lasair6.sh up        start the validators, a reader and the builder (images cached)
#   nets/lasair6.sh deploy    build both services with jam080-build and deploy them through
#                             the Bootstrap service; ids land in out/state/*.json
#   nets/lasair6.sh ctl ...   nets/jamctl.py inside the net (submit | read | head)
#   nets/lasair6.sh down      stop the net and wipe the chain
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/.." && pwd); SUB=$(cd "$ROOT/.." && pwd)
JAMSWAP=$SUB/jamswap; COMPOSE="docker compose -f $JAMSWAP/docker-compose.lasair6.yml"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
PY="docker run --rm -i --network jamswap_lasnet -v $JAMSWAP/offchain:/offchain:ro -v jamswap_shared:/shared:ro -v $ROOT/out:/work -v $HERE/jamctl.py:/jamctl.py:ro python:3.11-slim"
case "${1:-}" in
  up)   (cd "$JAMSWAP" && $COMPOSE up -d spec-init lm0 lm1 lm2 lm3 lm4 lm5 reader builder) ;;
  down) (cd "$JAMSWAP" && $COMPOSE down -v) ;;
  build) mkdir -p "$ROOT/out"; "$BUILD" "$ROOT/services/public-balance" --out "$ROOT/out"; "$BUILD" "$ROOT/services/privacy" --out "$ROOT/out" ;;
  deploy)
    "$0" build; mkdir -p "$ROOT/out/state"
    for svc in public-balance privacy; do
      $PY python3 /offchain/deploy.py --rpc ws://reader:19800 --chain-spec /shared/spec.json \
        --code /work/$svc-service.jam --state /work/state/$svc.json --no-setup --timeout 900 | tail -1
    done ;;
  ctl) shift; $PY python3 /jamctl.py "$@" ;;
  *) sed -n 2,8p "$0"; exit 1 ;;
esac
