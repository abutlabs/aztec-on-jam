#!/usr/bin/env bash
# #41: Poseidon2 (BN254, t = 4; Noir's permutation and noir-lang/poseidon's sponge) in the PVM.
# Can a privacy service's accumulate (10M gas a report, GP 0.8.0) afford a tree's insertions?
# p2/ is a no_std port of client/poseidon2.py; host/ holds it to the same vectors (Noir's
# permutation vectors, hash_cli's outputs, the empty depth-16 root). The expected results
# below come from client/poseidon2.py, a second implementation. Then on a PolkaJam testnet
# (spikes/lib/polkajam.sh): chained hashes and depth-40 paths; refine's gas is the PVM's,
# the same as accumulate would pay. #43: k leaves appended to an append-only depth-40 tree in
# one refine, level by level; expected roots from tools/append.py's per-leaf insertion, a
# different algorithm, itself held to a naive full tree.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$HERE/out; mkdir -p "$O"
. "$HERE/../lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build

echo "== 1. on the host"
(cd "$HERE/host" && cargo run -q --release -- "$HERE/../../client/poseidon2-bn254.json")

PYTHONPATH="$HERE/../../client:$HERE/tools" python3 - "$O" <<'PY'
import random, sys
import poseidon2 as p2
import append
out = sys.argv[1]
random.seed(41)
be = lambda x: x.to_bytes(32, "big")
def chained(n):
    x = 1
    for _ in range(n):
        x = p2.hash([x, x])
    return x
def path_root(leaf, index, sibs):
    node = leaf
    for level, s in enumerate(sibs):
        node = p2.hash([s, node]) if (index >> level) & 1 else p2.hash([node, s])
    return node
leaf = random.randrange(p2.P); index = random.randrange(1 << 40); sibs = [random.randrange(p2.P) for _ in range(40)]
root = path_root(leaf, index, sibs)
path = be(leaf) + index.to_bytes(8, "little") + b"".join(be(s) for s in sibs)
cases = {1: bytes([0]) + (1).to_bytes(2, "little") + be(chained(1)),
         2: bytes([0]) + (10).to_bytes(2, "little") + be(chained(10)),
         3: bytes([0]) + (100).to_bytes(2, "little") + be(chained(100)),
         4: bytes([1]) + (1).to_bytes(2, "little") + path + be(root),
         5: bytes([1]) + (10).to_bytes(2, "little") + path + be(root),
         6: bytes([1]) + (1).to_bytes(2, "little") + path + be((root + 1) % p2.P)}
assert append.check(4)
zero = append.zeros(40)
start = random.randrange(1 << 39, 1 << 40) | 1   # odd, so the frontier is used from level 0
frontier = [random.randrange(p2.P) for _ in range(40)]
leaves = [random.randrange(p2.P) for _ in range(2048)]
state = start.to_bytes(8, "little") + b"".join(be(x) for x in frontier) + b"".join(be(x) for x in zero)
for t, k in [(7, 1), (8, 64), (9, 1024), (10, 2048)]:
    want = append.sequential(frontier, zero, start, leaves[:k])
    assert append.batch(frontier, zero, start, leaves[:k]) == want
    cases[t] = bytes([2]) + k.to_bytes(2, "little") + state + b"".join(be(x) for x in leaves[:k]) + be(want)
cases[11] = cases[10][:-32] + be((int.from_bytes(cases[10][-32:], "big") + 1) % p2.P)
for t, b in cases.items():
    open(f"{out}/p{t}.bin", "wb").write(bytes([t]) + b)
PY

echo "== 2. on a PolkaJam testnet ($("$PJD/polkajam" --version | head -1))"
BLOB=$("$BUILD" "$HERE/service" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
for t in $(seq 1 11); do pj_send "$t" "$O/p$t.bin"; done
sleep 12
names=("" "1 hash" "10 hashes, chained" "100 hashes, chained" "1 depth-40 path (40 hashes)" "10 depth-40 paths" "1 path, wrong root expected (must fail)"
       "append 1 leaf to a depth-40 tree" "append 64 leaves" "append 1,024 leaves" "append 2,048 leaves" "append 2,048 leaves, wrong root expected (must fail)")
echo
echo "| case | refine gas | PolkaJam refine ($(pj_backend), $(uname -sm)) | accumulate's verdict |"
echo "|---|---|---|---|"
for t in $(seq 1 11); do
  key=$(printf '763a%02x' "$t")
  rows=$(pj_refines "$t" || true)
  [ -n "$rows" ] || { printf '| %s | refused | | %s |\n' "${names[$t]}" "$(pj_read "$key")"; continue; }
  g=$(echo "$rows" | awk '{print $1}' | sort -n | head -1)
  med=$(echo "$rows" | awk '{print $2}' | sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}')
  printf '| %s | %s | %s s of %s | %s |\n' "${names[$t]}" "$(python3 -c "print(f'{$g:,}')")" "$med" "$(echo "$rows" | wc -l | tr -d ' ')" "$(pj_read "$key")"
done
