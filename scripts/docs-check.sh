#!/usr/bin/env bash
# The docs reviewer (Aodh, 2026-10-28): the Pages site must not carry retired approaches or
# raw jargon. Run before every push that touches docs/; exit 1 names each stale line.
#   scripts/docs-check.sh
set -uo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd)
FAIL=0
check() { # <pattern> <why> [extra grep args...]
  local pat=$1 why=$2; shift 2
  local hits
  hits=$(grep -rn "$@" -E "$pat" "$HERE"/docs/index.html "$HERE"/docs/*/index.html "$HERE"/docs/site.css 2>/dev/null)
  if [ -n "$hits" ]; then FAIL=1; echo "STALE ($why):"; echo "$hits" | head -5; fi
}
check "Today's demo|JAM Rebel Wallet runs on our own stack|retired by the move" "retired approaches stay out of the overview"
check "JAM PRIVACY" "the crawl says AZTEC ON JAM now"
grep -rn -E "call 10[1-5]|calls 10[1-5]|101–10[45]" "$HERE"/docs/index.html "$HERE"/docs/aztec/index.html "$HERE"/docs/demo/index.html "$HERE"/docs/operators/index.html 2>/dev/null | grep -v "<td>" | grep -q . && { FAIL=1; echo "STALE (say 'the JIP' in prose; numbers live in tables and the JIP page):"; grep -rn -E "call 10[1-5]|calls 10[1-5]|101–10[45]" "$HERE"/docs/index.html "$HERE"/docs/aztec/index.html "$HERE"/docs/demo/index.html "$HERE"/docs/operators/index.html | grep -v "<td>" | head -5; }
check "research path|not needed for path A|optional path" "refine checking the work is the point, never optional"
check "no curve call<" "the per-tile chip was removed; the legend says it once"
check "REVISE|TODO|FIXME|\?\?|updated 20[0-9][0-9]|Done 20[0-9][0-9]|still being wired|grows as they land|setTimeout" "developer notes stay off the public pages"
check ">#[0-9]{2,3}<" "issue numbers stay off the public pages (link the roadmap in words)"
check "facade" "say 'JAM sidecar' on the public pages"
# the JIP is a final copy: no drafting notes, dates, tracker numbers or repository paths
hits=$(grep -nE "\*Status:|README line|simulated|CI run|#[0-9]{2}\b|20[0-9]{2}-[0-9]{2}-[0-9]{2}|spikes/|crates/|should say so|should change|Change log" "$HERE/docs/jip/jip-final.md")
if [ -n "$hits" ]; then FAIL=1; echo "STALE (the JIP carries drafting notes):"; echo "$hits" | head -5; fi
[ $FAIL = 0 ] && echo "docs-check: clean"
exit $FAIL
