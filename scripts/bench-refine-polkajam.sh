#!/usr/bin/env bash
# PolkaJam's refine of one privacy proof, timed by PolkaJam itself (its JIP-3 `Refined` events,
# read by its own jam-telemetry-dumper): a local polkajam-testnet (six validators),
# the privacy service deployed with `jamboree rpc`, one proved payload submitted N times. Dummy
# finality: refine is what is measured, and on a small CI runner GRANDPA can take minutes to
# leave genesis, before which a package's anchor is refused.
# Refine reads no state, so any valid payload refines in full on any chain.
#   scripts/bench-refine-polkajam.sh <polkajam release dir> <payload.hex> <privacy-service.jam>
# Env: N (packages, default 5), JAMBOREE (default jamboree on PATH). Prints one summary line:
# the PVM backend PolkaJam chose (its recompiler on x86-64 Linux, else its interpreter).
set -euo pipefail
PJD=$(cd "${1:?polkajam release dir}" && pwd); PAYLOAD=${2:?payload hex}; BLOB=${3:?privacy-service.jam}
JB=${JAMBOREE:-jamboree}; N=${N:-5}; W=$(mktemp -d)
cleanup() { pkill -f "$PJD/polkajam" 2>/dev/null || true; pkill -f "$PJD/jam-telemetry-dumper" 2>/dev/null || true; }
trap cleanup EXIT
"$PJD/polkajam" --chain dev dump-spec "$W/spec.json" >/dev/null
"$PJD/jam-telemetry-dumper" --port 19900 > "$W/telemetry.log" 2>&1 &
(cd "$W" && "$PJD/polkajam-testnet" --telemetry 127.0.0.1:19900 --finality-mode dummy > "$W/testnet.log" 2>&1 &)
rpc() { "$JB" rpc --url ws://127.0.0.1:19800 --chain-spec "$W/spec.json" "$@"; }
for _ in $(seq 1 150); do rpc head 2>/dev/null | grep -q "finalized [1-9]" && break; sleep 2; done
rpc head >/dev/null 2>&1 || { echo "the testnet never answered on :19800; its log:" >&2; tail -40 "$W/testnet.log" >&2; exit 1; }
rpc deploy "$BLOB" --state "$W/privacy.json" | tail -1 >&2
SID=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['service_id'])" "$W/privacy.json")
for _ in $(seq 1 "$N"); do rpc submit "$SID" "0x$(tr -d '\n' < "$PAYLOAD")" --wait --timeout 120 >/dev/null 2>&1 || true; done
sleep 3
BACKEND=$(grep -m1 -o "PVM backend: [a-z]*" "$W/testnet.log" | sed 's/PVM backend: //')
VERSION=$("$PJD/polkajam" --version | head -1)
python3 - "$W/telemetry.log" "$BACKEND" "$VERSION" "$(uname -sm)" <<'PY'
import re, statistics as st, sys
log, backend, version, host = sys.argv[1:]
rows = [(int(g), int(ns) / 1e9, node) for node, g, ns in
        re.findall(r"^(\S+?)@\S+: \S+: Refined \{.*?RefineCost \{ total: ExecCost \{ gas: (\d+), ns: (\d+) \}",
                   open(log).read(), re.M)
        if int(g) > 100_000_000]
if not rows:
    sys.exit("no privacy refine in the telemetry")
t = sorted(r[1] for r in rows)
print(f"{version} ({backend}, {host}): {len(rows)} refines of one privacy proof "
      f"({rows[0][0]:,} gas): median {st.median(t):.3f} s, min {t[0]:.3f}, max {t[-1]:.3f}")
nodes = {}
for g, t_, node in rows:
    nodes.setdefault(node[:8], []).append(t_)
print("  each node's refines, in order:", "; ".join(f"{n}: " + " ".join(f"{x:.3f}" for x in ts) for n, ts in nodes.items()))
PY
