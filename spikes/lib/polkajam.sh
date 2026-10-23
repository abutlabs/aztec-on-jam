# Sourced by a spike's measure.sh: one six-validator PolkaJam testnet (v0.8.0, dummy finality),
# a spike service deployed with `jamboree rpc`, payloads sent one package at a time, and
# PolkaJam's own JIP-3 telemetry for each package's refines (gas, time).
#   pj_start <out dir>        start the dumper and the testnet; wait for finality
#   pj_deploy <blob>          deploy; sets PJ_SID
#   pj_send <tag> <file>      one package, from a .hex file or raw bytes (anything else, sent
#                             as @file); its Refined events go to $PJ_OUT/refined-<tag>.log
#   pj_read <key hex>         a storage value of PJ_SID
#   pj_refines <tag>          "gas seconds" per refine of that package
# The testnet stops when the sourcing script exits.
SUB=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)
case "$(uname -s)-$(uname -m)" in Darwin-arm64) PJP=macos-aarch64 ;; Linux-aarch64) PJP=linux-aarch64 ;; *) PJP=linux-x86_64 ;; esac
PJD=${PJD:-$HOME/.cache/jam-privacy/polkajam-v0.8.0/polkajam-v0.8.0-$PJP}
JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}
PJ_W=$(mktemp -d)

pj_stop() { pkill -f "$PJD/polkajam" 2>/dev/null || true; pkill -f "$PJD/jam-telemetry-dumper" 2>/dev/null || true; }
trap pj_stop EXIT
pj_rpc() { "$JB" rpc --url ws://127.0.0.1:19800 --chain-spec "$PJ_W/spec.json" "$@"; }

pj_start() {
  PJ_OUT=$1; mkdir -p "$PJ_OUT"
  "$PJD/polkajam" --chain dev dump-spec "$PJ_W/spec.json" > /dev/null
  "$PJD/jam-telemetry-dumper" --port 19900 > "$PJ_OUT/telemetry.log" 2>&1 &
  (cd "$PJ_W" && "$PJD/polkajam-testnet" --telemetry 127.0.0.1:19900 --finality-mode dummy > "$PJ_OUT/testnet.log" 2>&1 &)
  # ready once finality has left genesis and the nodes see each other: a node can finalize
  # alone under dummy finality, before any guarantor is reachable
  for _ in $(seq 1 150); do
    if pj_rpc head 2>/dev/null | grep -q "finalized [1-9]" &&
       [ "$(grep -oE '^node[0-9]+: .*Net status: [5-9] peers' "$PJ_OUT/testnet.log" | cut -d: -f1 | sort -u | wc -l)" -ge 6 ]; then
      return
    fi
    sleep 2
  done
  echo "the PolkaJam testnet never finalized with its nodes connected; its log:" >&2; tail -30 "$PJ_OUT/testnet.log" >&2; exit 1
}

pj_deploy() {
  pj_rpc deploy "$1" --state "$PJ_W/svc.json" 2>&1 | tail -1
  PJ_SID=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['service_id'])" "$PJ_W/svc.json")
}

pj_send() {
  local from; from=$(wc -l < "$PJ_OUT/telemetry.log")
  local payload; case $2 in *.hex) payload="0x$(cat "$2")" ;; *) payload="@$2" ;; esac
  pj_rpc submit "$PJ_SID" "$payload" --wait --timeout 180 > "$PJ_OUT/submit-$1.json" 2>&1 || echo "  $1: submit: $(tail -1 "$PJ_OUT/submit-$1.json")"
  sleep 4
  tail -n +"$((from + 1))" "$PJ_OUT/telemetry.log" | grep "Refined {" > "$PJ_OUT/refined-$1.log" || true
}

pj_read() { pj_rpc read "$PJ_SID" "0x$1" 2>/dev/null | tail -1; }

pj_backend() { grep -m1 -o 'PVM backend: [a-z]*' "$PJ_OUT/testnet.log" | sed 's/PVM backend: //'; }

pj_refines() {
  grep -oE "RefineCost \{ total: ExecCost \{ gas: [0-9]+, ns: [0-9]+" "$PJ_OUT/refined-$1.log" \
    | awk -F'[:,]' '{printf "%s %.4f\n", $3+0, $5/1e9}'
}
