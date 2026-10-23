#!/usr/bin/env bash
# The final verification: on each net, from a clean state, `down`, `up`, `check`, the three
# stage commands and the two-browser run; then jamboree's head check on each native net.
# One summary line per step; full logs in out/verify/.  RECORD_ON=<net> records the browsers there.
#   scripts/verify-all.sh [net ...]     (default: lasair6 pj6 jb-pj6 jb-lasair6 jb-mixed)
set -uo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd); cd "$ROOT"; V=$ROOT/out/verify; mkdir -p "$V"
NETS=${*:-lasair6 pj6 jb-pj6 jb-lasair6 jb-mixed}; fails=0
step() { # <net> <name> <command...>: run, time, one line
  local net=$1 name=$2; shift 2; local t0=$SECONDS log="$V/$net-$name.log"
  if NET=$net "$@" > "$log" 2>&1; then r=ok; else r=FAIL; fails=$((fails + 1)); fi
  printf '%-11s %-9s %-4s %4ds  %-62s %s\n' "$net" "$name" "$r" $((SECONDS - t0)) "$(grep -E 'PASS|FAIL|holds|indexer up|same head' "$log" | tail -1 | cut -c1-62)" \
    "$(n=$(grep -c 'sending the package again' "$log"); [ "$n" -gt 0 ] && echo "($n resent)")"
}
for net in lasair6 pj6 jb-pj6 jb-lasair6 jb-mixed; do NET=$net nets/demo.sh down >/dev/null 2>&1; done
for net in $NETS; do
  rm -rf out/state out/client-*.json out/stage-*.json
  step "$net" down nets/demo.sh down
  step "$net" up nets/demo.sh up
  step "$net" check nets/demo.sh check
  step "$net" shield nets/demo.sh shield
  step "$net" send nets/demo.sh send
  step "$net" unshield nets/demo.sh unshield
  if [ "${RECORD_ON:-}" = "$net" ]; then step "$net" e2e env RECORD=1 node client/e2e/e2e.mjs   # the README's images
  else step "$net" e2e env IMAGES="$V/images-$net" node client/e2e/e2e.mjs; fi
  step "$net" down nets/demo.sh down
done
for net in $NETS; do case "$net" in jb-*) step "$net" headcheck nets/jb.sh headcheck 3 ;; esac; done
echo "verify-all: $fails failed"; exit $((fails > 0))
