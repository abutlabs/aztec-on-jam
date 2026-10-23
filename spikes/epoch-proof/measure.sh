#!/usr/bin/env bash
# Can JAM check Aztec's epoch proof in refine, and at what cost? Aztec's root rollup proof is a
# plain UltraHonk proof with a Keccak transcript (bb `-t evm-no-zk`), 2^24 gates, 113 public
# inputs plus the 8 pairing-point limbs (l1-contracts/script/deploy/HonkVerifier.sol).
#   1. Synthetic circuits of 2^LOGS (default 12 14 16 18 20 22) with 113 public inputs, proved
#      as Aztec proves the root rollup (Noir 1.0.0-rc.3 and bb 6.0.0, Aztec v6's pins); each
#      checked by `bb verify` and by host/ (our verifier, ported to bb 6.0.0) before use.
#   2. One six-validator PolkaJam testnet (v0.8.0, dummy finality), service/ deployed with
#      `jamboree rpc`, each proof submitted in its own package. PolkaJam's JIP-3 `Refined` events
#      (its jam-telemetry-dumper) give refine's gas and time; accumulate stores each verdict,
#      read back with `jamboree rpc read`. Plus one proof with a public input altered.
# Writes out/; prints a table.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); SUB=$(cd "$HERE/../../.." && pwd); O=$HERE/out; mkdir -p "$O"
NARGO=$HOME/.nargo/versions/nargo-1.0.0-rc.3; BB=$HOME/.bb/versions/bb-6.0.0
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}
case "$(uname -s)-$(uname -m)" in Darwin-arm64) PJP=macos-aarch64 ;; Linux-aarch64) PJP=linux-aarch64 ;; *) PJP=linux-x86_64 ;; esac
PJD=$HOME/.cache/jam-privacy/polkajam-v0.8.0/polkajam-v0.8.0-$PJP
LOGS=${LOGS:-12 14 16 18 20 22}; NPUBS=113

echo "== 1. proofs (nargo $("$NARGO" --version | head -1 | awk '{print $4}'), bb $("$BB" --version))"
(cd "$HERE/host" && cargo build -q --release)
for k in $LOGS; do
  d=$O/c$k; mkdir -p "$d/src"
  if [ ! -f "$d/proof/proof" ]; then
    # N squarings, one gate each, sized to fill 2^k after the fixed rows (calibrated by `bb gates`)
    n=$(( (1 << k) - 600 ))
    printf '[package]\nname = "c%s"\ntype = "bin"\nauthors = [""]\n\n[dependencies]\n' "$k" > "$d/Nargo.toml"
    cat > "$d/src/main.nr" <<EOF
global N: u32 = $n;

fn main(x: Field, inputs: pub [Field; $((NPUBS - 1))]) -> pub Field {
    let mut acc = x;
    for i in 0..N {
        acc = acc * acc + inputs[i % $((NPUBS - 1))];
    }
    acc
}
EOF
    python3 -c "print('x = \"3\"'); print('inputs = [' + ', '.join('\"%d\"' % (i + 1) for i in range($NPUBS - 1)) + ']')" > "$d/Prover.toml"
    (cd "$d" && "$NARGO" execute --silence-warnings > /dev/null)
    mkdir -p "$d/proof"
    /usr/bin/time -l "$BB" write_vk -t evm-no-zk -b "$d/target/c$k.json" -o "$d/proof" > "$d/vk.log" 2>&1
    /usr/bin/time -l "$BB" prove -t evm-no-zk -b "$d/target/c$k.json" -w "$d/target/c$k.gz" -k "$d/proof/vk" -o "$d/proof" > "$d/prove.log" 2>&1
    "$BB" verify -t evm-no-zk -p "$d/proof/proof" -i "$d/proof/public_inputs" -k "$d/proof/vk" > /dev/null 2>&1
  fi
  logn=$(python3 -c "import sys;print(int.from_bytes(open(sys.argv[1],'rb').read()[:32],'big'))" "$d/proof/vk")
  [ "$logn" = "$k" ] || { echo "c$k: the vk says log n $logn" >&2; exit 1; }
  host=$("$HERE/host/target/release/epoch-proof-host" "$d/proof" | tail -1)
  [ "$host" = accepted ] || { echo "c$k: our verifier on the host: $host" >&2; exit 1; }
  printf '  2^%-3s proof %6s B, %s public inputs, prove %6s s, peak %5s MB; bb and our verifier accept\n' "$k" \
    "$(wc -c < "$d/proof/proof" | tr -d ' ')" "$(( $(wc -c < "$d/proof/public_inputs") / 32 ))" \
    "$(awk '/ real/{print $1}' "$d/prove.log")" "$(awk '/maximum resident/{printf "%d", $1/1048576}' "$d/prove.log")"
done

payload() { # <dir> <tag> [tamper]: [0 plain][tag][vk_len][vk][n][pubs][proof] as hex
  python3 - "$1" "$2" "${3:-}" <<'PY'
import sys
d, tag, tamper = sys.argv[1], int(sys.argv[2]), sys.argv[3]
vk = open(f"{d}/proof/vk", "rb").read(); pubs = bytearray(open(f"{d}/proof/public_inputs", "rb").read())
if tamper: pubs[31] ^= 1
proof = open(f"{d}/proof/proof", "rb").read()
p = bytes([0, tag]) + len(vk).to_bytes(2, "little") + vk + (len(pubs) // 32).to_bytes(2, "little") + pubs + proof
print(p.hex())
PY
}

echo "== 2. on a PolkaJam testnet ($("$PJD/polkajam" --version | head -1))"
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
W=$(mktemp -d)
cleanup() { pkill -f "$PJD/polkajam" 2>/dev/null || true; pkill -f "$PJD/jam-telemetry-dumper" 2>/dev/null || true; }
trap cleanup EXIT
"$PJD/polkajam" --chain dev dump-spec "$W/spec.json" > /dev/null
"$PJD/jam-telemetry-dumper" --port 19900 > "$O/telemetry.log" 2>&1 &
(cd "$W" && "$PJD/polkajam-testnet" --telemetry 127.0.0.1:19900 --finality-mode dummy > "$O/testnet.log" 2>&1 &)
rpc() { "$JB" rpc --url ws://127.0.0.1:19800 --chain-spec "$W/spec.json" "$@"; }
for _ in $(seq 1 150); do rpc head 2>/dev/null | grep -q "finalized [1-9]" && break; sleep 2; done
rpc deploy "$BLOB" --state "$W/svc.json" 2>&1 | tail -1
SID=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['service_id'])" "$W/svc.json")
# one package at a time: the Refined events logged while it is in flight are its guarantors'
send() { # <tag> <hex file>
  local from; from=$(wc -l < "$O/telemetry.log")
  rpc submit "$SID" "0x$(cat "$2")" --wait --timeout 180 > "$O/submit-$1.json" 2>&1 || echo "  $1: submit: $(tail -1 "$O/submit-$1.json")"
  sleep 4
  tail -n +"$((from + 1))" "$O/telemetry.log" | grep "Refined {" > "$O/refined-$1.log" || true
}
for k in $LOGS; do payload "$O/c$k" "$k" > "$W/p$k.hex"; send "$k" "$W/p$k.hex"; done
last=$(echo $LOGS | awk '{print $NF}')
payload "$O/c$last" 99 tamper > "$W/bad.hex"; send 99 "$W/bad.hex"
sleep 12
verdict() { rpc read "$SID" "0x763a$(printf '%02x' "$1")" 2>/dev/null | tail -1; }
for k in $LOGS 99; do echo "$k $(verdict "$k")"; done > "$O/verdicts.txt"

python3 - "$O" "$LOGS" "$(uname -sm)" "$(grep -m1 -o 'PVM backend: [a-z]*' "$O/testnet.log" | sed 's/PVM backend: //')" <<'PY'
import re, statistics as st, sys
out, logs, host, backend = sys.argv[1:]
verdicts = dict(l.split(maxsplit=1) for l in open(f"{out}/verdicts.txt").read().splitlines() if " " in l)
def refines(tag):
    rows = re.findall(r"RefineCost \{ total: ExecCost \{ gas: (\d+), ns: (\d+) \}", open(f"{out}/refined-{tag}.log").read())
    return [(int(g), int(ns) / 1e9) for g, ns in rows]
print(f"\n| circuit | refine gas | of a tiny package (1e9) | of a full one (5e9) | PolkaJam refine ({backend}, {host}) | accumulate's verdict |")
print("|---|---|---|---|---|---|")
for k in logs.split() + ["99"]:
    r = refines(k)
    name = f"2^{k}, 113 public inputs" if k != "99" else f"2^{logs.split()[-1]}, a public input altered"
    gas = sorted({g for g, _ in r})
    g = gas[0] if len(gas) == 1 else None
    cell = f"{g:,}" if g is not None else (", ".join(f"{x:,}" for x in gas) or "none")
    share = (f"{g/1e9:.1%}", f"{g/5e9:.1%}") if g else ("", "")
    times = f"median {st.median(t for _, t in r):.3f} s of {len(r)}" if r else ""
    print(f"| {name} | {cell} | {share[0]} | {share[1]} | {times} | {verdicts.get(k, '?')} |")
PY
