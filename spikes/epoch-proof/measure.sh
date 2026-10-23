#!/usr/bin/env bash
# Can JAM check Aztec's epoch proof in refine, and at what cost? Aztec's root rollup proof is a
# plain UltraHonk proof with a Keccak transcript (bb `-t evm-no-zk`), 2^24 gates, 113 public
# inputs plus the 8 pairing-point limbs (l1-contracts/script/deploy/HonkVerifier.sol).
#   1. Synthetic circuits of 2^LOGS (default 12 14 16 18 20 22) with 113 public inputs, proved
#      as Aztec proves the root rollup (Noir 1.0.0-rc.3 and bb 6.0.0, Aztec v6's pins); each
#      checked by `bb verify` and by host/ (our verifier, ported to bb 6.0.0) before use.
#   2. One six-validator PolkaJam testnet (spikes/lib/polkajam.sh), service/ deployed, each
#      proof submitted in its own package. PolkaJam's JIP-3 `Refined` events
#      (its jam-telemetry-dumper) give refine's gas and time; accumulate stores each verdict,
#      read back with `jamboree rpc read`. Plus one proof with a public input altered.
# Writes out/; prints a table.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
NARGO=$HOME/.nargo/versions/nargo-1.0.0-rc.3; BB=$HOME/.bb/versions/bb-6.0.0
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
LOGS=${LOGS:-12 14 16 18 20 22}; NPUBS=113
# CIRCUIT=rich: every UltraHonk relation in use (lookups, Poseidon2, elliptic, ROM and RAM,
# non-native field, range), so no selector commitment is zero, as in a real root rollup (#23)
CIRCUIT=${CIRCUIT:-plain}; C=$([ "$CIRCUIT" = rich ] && echo r || echo c)

echo "== 1. proofs (nargo $("$NARGO" --version | head -1 | awk '{print $4}'), bb $("$BB" --version))"
(cd "$HERE/host" && cargo build -q --release)
for k in $LOGS; do
  d=$O/$C$k; mkdir -p "$d/src"
  if [ ! -f "$d/proof/proof" ]; then
    printf '[package]\nname = "%s%s"\ntype = "bin"\nauthors = [""]\n\n[dependencies]\n' "$C" "$k" > "$d/Nargo.toml"
    if [ "$CIRCUIT" = rich ]; then
    # blocks of about 17,500 gates on about 43,400 fixed (bb gates, 1 and 4 blocks), filling 3/4 of 2^k
    n=$(( ((1 << (k - 1)) + (1 << (k - 2)) - 43362) / 17524 )); [ "$n" -ge 1 ] || n=1
    cat > "$d/src/main.nr" <<EOF
use std::embedded_curve_ops::{EmbeddedCurvePoint, embedded_curve_add};

global N: u32 = $n;

fn main(x: u64, i: u32, pk_x: [u8; 32], pk_y: [u8; 32], sig: [u8; 64], msg: [u8; 32], inputs: pub [Field; $((NPUBS - 1))]) -> pub Field {
    let mut s: [u64; 25] = [x; 25];
    let mut p: [Field; 4] = [x as Field, 1, 2, 3];
    let g = EmbeddedCurvePoint { x: 1, y: 17631683881184975370165255887551781615748388533673675138860 };
    let mut q = g;
    let mut ram: [Field; 32] = [0; 32];
    let mut acc: Field = 0;
    for k in 0..N {
        s[0] = s[0] ^ (inputs[k % $((NPUBS - 1))] as u64);
        s = std::hash::keccakf1600(s);
        p = std::hash::poseidon2_permutation(p);
        q = embedded_curve_add(q, g);
        let j = (i + k) % 32;
        ram[j] = ram[j] + p[0];
        acc = acc + inputs[(i + k) % $((NPUBS - 1))] + ram[(j + 7) % 32] + ((s[1] as u8) as Field);
    }
    let ok = std::ecdsa_secp256k1::verify_signature(pk_x, pk_y, sig, msg);
    acc + q.x + (ok as Field)
}
EOF
    elif [ "$k" -le 22 ]; then
    # N squarings, one gate each, sized to fill 2^k after the fixed rows (calibrated by `bb gates`)
    n=$(( (1 << k) - 600 ))
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
    else
    # From 2^23 a loop that size unrolls into a program too big to compile here (#23: 2^24
    # swapped for 50 minutes in nargo execute). keccakf1600 is one opcode of about 15,100 gates
    # (25,500 fixed; bb gates, 1 and 10 calls), so N calls fill 3/4 of 2^k with a small program.
    n=$(( ((1 << (k - 1)) + (1 << (k - 2))) / 15117 ))
    cat > "$d/src/main.nr" <<EOF
global N: u32 = $n;

fn main(x: u64, inputs: pub [Field; $((NPUBS - 1))]) -> pub Field {
    let mut s: [u64; 25] = [x; 25];
    for i in 0..N {
        s[0] = s[0] ^ (inputs[i % $((NPUBS - 1))] as u64);
        s = std::hash::keccakf1600(s);
    }
    s[0] as Field
}
EOF
    fi
    python3 - "$d/Prover.toml" "$CIRCUIT" "$NPUBS" <<'PY'
import sys
path, circuit, npubs = sys.argv[1], sys.argv[2], int(sys.argv[3])
arr = lambda v, n: "[" + ", ".join([f'"{v}"'] * n) + "]"
lines = ['x = "3"', "inputs = [" + ", ".join(f'"{i + 1}"' for i in range(npubs - 1)) + "]"]
if circuit == "rich":
    lines += ['i = "5"', f"pk_x = {arr(1, 32)}", f"pk_y = {arr(2, 32)}", f"sig = {arr(3, 64)}", f"msg = {arr(4, 32)}"]
open(path, "w").write("\n".join(lines) + "\n")
PY
    (cd "$d" && "$NARGO" execute --silence-warnings > /dev/null)
    mkdir -p "$d/proof"
    /usr/bin/time -l "$BB" write_vk -t evm-no-zk -b "$d/target/$C$k.json" -o "$d/proof" > "$d/vk.log" 2>&1
    /usr/bin/time -l "$BB" prove -t evm-no-zk -b "$d/target/$C$k.json" -w "$d/target/$C$k.gz" -k "$d/proof/vk" -o "$d/proof" > "$d/prove.log" 2>&1
    "$BB" verify -t evm-no-zk -p "$d/proof/proof" -i "$d/proof/public_inputs" -k "$d/proof/vk" > /dev/null 2>&1
  fi
  logn=$(python3 -c "import sys;print(int.from_bytes(open(sys.argv[1],'rb').read()[:32],'big'))" "$d/proof/vk")
  [ "$logn" = "$k" ] || { echo "$C$k: the vk says log n $logn" >&2; exit 1; }
  host=$("$HERE/host/target/release/epoch-proof-host" "$d/proof" | tail -1)
  [ "$host" = accepted ] || { echo "$C$k: our verifier on the host: $host" >&2; exit 1; }
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
pj_start "$O"
pj_deploy "$BLOB"
for k in $LOGS; do payload "$O/$C$k" "$k" > "$O/p$k.hex"; pj_send "$k" "$O/p$k.hex"; done
last=$(echo $LOGS | awk '{print $NF}')
payload "$O/$C$last" 99 tamper > "$O/bad.hex"; pj_send 99 "$O/bad.hex"
sleep 12
for k in $LOGS 99; do echo "$k $(pj_read "763a$(printf '%02x' "$k")")"; done > "$O/verdicts.txt"

python3 - "$O" "$LOGS" "$(uname -sm)" "$(pj_backend)" "$CIRCUIT" <<'PY'
import re, statistics as st, sys
out, logs, host, backend, circuit = sys.argv[1:]
verdicts = dict(l.split(maxsplit=1) for l in open(f"{out}/verdicts.txt").read().splitlines() if " " in l)
def refines(tag):
    rows = re.findall(r"RefineCost \{ total: ExecCost \{ gas: (\d+), ns: (\d+) \}", open(f"{out}/refined-{tag}.log").read())
    return [(int(g), int(ns) / 1e9) for g, ns in rows]
print(f"\n| circuit | refine gas | of a tiny package (1e9) | of a full one (5e9) | PolkaJam refine ({backend}, {host}) | accumulate's verdict |")
print("|---|---|---|---|---|---|")
for k in logs.split() + ["99"]:
    r = refines(k)
    kind = "every relation" if circuit == "rich" else ("keccakf1600" if int(logs.split()[0] if k == "99" else k) >= 23 else "squarings")
    name = f"2^{k}, {kind}, 113 public inputs" if k != "99" else f"2^{logs.split()[-1]}, {kind}, a public input altered"
    gas = sorted({g for g, _ in r})
    g = gas[0] if len(gas) == 1 else None
    cell = f"{g:,}" if g is not None else (", ".join(f"{x:,}" for x in gas) or "none")
    share = (f"{g/1e9:.1%}", f"{g/5e9:.1%}") if g else ("", "")
    times = f"median {st.median(t for _, t in r):.3f} s of {len(r)}" if r else ""
    print(f"| {name} | {cell} | {share[0]} | {share[1]} | {times} | {verdicts.get(k, '?')} |")
PY
