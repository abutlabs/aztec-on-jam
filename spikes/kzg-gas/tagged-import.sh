#!/usr/bin/env bash
# #38: why a package importing by work-package hash (the tagged form) is never reported when
# its exporter is not recent. GP 0.8.0 lets a report's segment-root lookup name only packages
# in the recent history (reporting_assurance.tex:401-423), H = 8 blocks (definitions.tex:269).
# Control: export 32 segments (measure.sh's case 11), import them by package hash at once, then
# again after more than 8 slots, and by segments root after the wait. Uses measure.sh's
# payloads (run it first).
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; T=$O/tagged; mkdir -p "$T"
. "$HERE/../lib/polkajam.sh"
for f in p11.bin x11_0.bin p18.bin kzg-gas-service.jam; do [ -f "$O/$f" ] || { echo "run measure.sh first ($f)" >&2; exit 1; }; done
retag() { cp "$O/p18.bin" "$T/p$1.bin"; printf "\\x$(printf %x "$1")" | dd of="$T/p$1.bin" bs=1 seek=0 count=1 conv=notrunc 2>/dev/null; }
for t in 21 22 23; do retag $t; done
slot() { python3 -c "import json,sys; print(json.loads(open(sys.argv[1]).readline())['anchor_slot'])" "$1"; }
pj_start "$T"
pj_deploy "$O/kzg-gas-service.jam"
pj_send 11 "$O/p11.bin" --extrinsic "@$O/x11_0.bin" --exports 32
P11=$(python3 -c "import json,sys; print(json.loads(open(sys.argv[1]).readline())['package_hash'])" "$T/submit-11.json")
R11=$(python3 -c "import json,sys; print(json.loads(open(sys.argv[1]).read().splitlines()[-1])['status']['Reported']['report_hash'])" "$T/submit-11.json")
pj_rpc report "$R11" > "$T/report-11.json"
ROOT11=$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['segments_root'])" "$T/report-11.json")
by_pkg=(); by_root=(); for i in $(seq 0 31); do by_pkg+=(--import "pkg:$P11:$i"); by_root+=(--import "root:$ROOT11:$i"); done
pj_send 21 "$T/p21.bin" "${by_pkg[@]}"
sleep 66   # 11 slots of 6 s: package 11 leaves the 8-block recent history
pj_send 22 "$T/p22.bin" "${by_pkg[@]}"
pj_send 23 "$T/p23.bin" "${by_root[@]}"
sleep 12
echo
echo "| package | imports | anchor slot (11's: $(slot "$T/submit-11.json")) | refines | outcome | stored verdict |"
echo "|---|---|---|---|---|---|"
for t in 21 22 23; do
  case $t in 21) how="by package hash, at once" ;; 22) how="by package hash, after 11 slots" ;; 23) how="by segments root, after 11 slots" ;; esac
  out=$(tail -1 "$T/submit-$t.json" | sed 's/|/\//g')
  printf '| %s | %s | %s | %s | %s | %s |\n' "$t" "$how" "$(slot "$T/submit-$t.json")" "$(pj_refines "$t" | wc -l | tr -d ' ')" "$out" "$(pj_read "$(printf '763a%02x' "$t")")"
done
echo
echo "secondaries' failures:"
grep -E "WorkPackageFailed|WorkPackageSharingFailed" "$T/telemetry.log" | sed -E 's/^.*: [0-9]+@[0-9]+: //' | sort | uniq -c || echo "  none"
