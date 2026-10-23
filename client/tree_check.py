#!/usr/bin/env python3
"""The client's trees, checked without a chain: every insert's witness opens the roots the
circuit checks, a tree built at once equals one built by inserts, a kept tree is rebuilt when
the chain's list diverges, the wallet's JS (../demo-wallet/jam/lib/core.js) builds the same
nullifier root, and building stays linear (it was quadratic: 400 nullifiers took 96 s, and
the hour's soak slowed to a quarter of a package a slot)."""
import json, random, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import privacy  # noqa: E402
from privacy import h, IndexedTree, Tree, P  # noqa: E402

CORE = HERE.parent.parent / "demo-wallet" / "jam" / "lib" / "core.js"


def fold(leaf, index, path):
    node = leaf
    for i, sib in enumerate(path):
        node = h(int(sib), node) if (index >> i) & 1 else h(node, int(sib))
    return node


random.seed(1)
nfs = [random.randrange(1, P) for _ in range(60)] + random.sample(range(1, 500), 40)
random.shuffle(nfs)

# every witness opens the roots: the low leaf under the old root, the empty slot under the root
# with the low leaf rewritten, the new leaf under the new root
t = IndexedTree()
for nf in nfs:
    old = t.root()
    low, slot = t.insert(nf)
    lo, new = int(low["index"]), len(t.leaves) - 1
    assert fold(h(int(low["value"]), int(low["next_index"]), int(low["next_value"])), lo, low["path"]) == old
    mid = fold(h(int(low["value"]), new, nf), lo, low["path"])
    assert fold(0, new, slot) == mid and fold(h(*t.leaves[new]), new, slot) == t.root()
    try:
        t.insert(nf); raise SystemExit(f"{nf:#x} inserted twice")
    except privacy.ClientError:
        pass
assert IndexedTree(nfs).root() == t.root() and IndexedTree(nfs).leaves == t.leaves
assert [v for v, _, _ in sorted(t.leaves)] == sorted([0] + nfs)
print(f"indexed tree: {len(nfs)} inserts, every witness opens its roots, duplicates refused, "
      "built at once = built by inserts")

leaves = [random.randrange(1, P) for _ in range(33)]
n = Tree(leaves[:3]); n.root()
for i in range(3, len(leaves)):
    n.set(i, leaves[i]); assert n.root() == Tree(leaves[:i + 1]).root()
n.set(7, 5); leaves[7] = 5; assert n.root() == Tree(leaves).root() and n.path(20) == Tree(leaves).path(20)
print("Tree.set: appends and rewrites equal a fresh tree")

k = privacy.nullifier_tree(nfs[:30]); k.insert(nfs[30])            # a proof that lands
assert privacy.nullifier_tree(nfs[:35]) is k and k.root() == IndexedTree(nfs[:35]).root()
k.insert(nfs[40])                                                   # one that never does
r = privacy.nullifier_tree(nfs[:36])
assert r is not k and r.root() == IndexedTree(nfs[:36]).root()
a = privacy.note_tree(leaves[:10])
assert privacy.note_tree(leaves[:20]) is a and a.root() == Tree(leaves[:20]).root()
assert privacy.note_tree(leaves[:9] + [1]).root() == Tree(leaves[:9] + [1]).root()
print("kept trees: extended while the chain's list extends them, rebuilt when it diverges")

many = [random.randrange(1, P) for _ in range(2000)]
t0 = time.time(); root = IndexedTree(many).root(); dt = time.time() - t0
assert dt < 20, f"2000 nullifiers took {dt:.1f} s: the build is no longer linear"
print(f"2000 nullifiers built in {dt:.2f} s")

js = subprocess.run(["node", "--input-type=module", "-e", f"""
  import * as W from {json.dumps(CORE.as_uri())};
  import {{ readFileSync }} from "node:fs";
  await W.init();
  const xs = JSON.parse(readFileSync(0, "utf8")).map(BigInt);
  const t0 = performance.now(); const r = new W.IndexedTree(xs).root();
  console.log(r.toString(), (performance.now() - t0).toFixed(0));"""],
    input=json.dumps([str(x) for x in many]), capture_output=True, text=True)
assert js.returncode == 0, js.stderr
js_root, js_ms = js.stdout.split()
assert int(js_root) == root, "the wallet's nullifier root differs from the client's"
print(f"the wallet's JS builds the same root ({js_ms} ms)")
