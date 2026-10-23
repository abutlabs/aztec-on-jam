#!/usr/bin/env python3
"""The demo's private-side client: notes, the note tree, witnesses, proofs, payloads.
Stdlib Python driving Noir (nargo) and Barretenberg (bb), so every hash it computes is the
circuit's own (circuits/hash_cli). State: a JSON file (CLIENT_STATE, default
client/state.json) of notes and the tree's leaves.

  privacy.py pk <owner>                               owner's public key (sk = H(owner) mod p)
  privacy.py deposit <owner> <value>                  a note for a shield; prints its commitment
  privacy.py transfer <sender> <in_a> <in_b> <to> <value> <unshield> <receiver> <dest> <out>
      in_a, in_b: note ids, "deposit:<id>" for a shield claim, or "-" for a dummy; the
      outputs are a note of <value> for <to> and the change for <sender> (minus <unshield>);
      <receiver>, <dest>: the public account (a small integer) and service id for the unshield;
      writes the work-item payload hex to <out> and marks the inputs spent
  privacy.py notes [owner]                            the unspent notes
  privacy.py root                                     the tree root the client believes
  privacy.py sync <leaves.json>                       take the tree's leaves and nullifier root
      from the chain ({"leaves": [hex...], "null_root": hex}, read by nets/demo.sh over JIP-2)
"""
import hashlib, json, os, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
CIRCUITS = ROOT / "circuits"
STATE = Path(os.environ.get("CLIENT_STATE", HERE / "state.json"))
DEPTH = 16
P = 21888242871839275222246405745257275088548364400416034343698204186575808495617
ENV = dict(os.environ, PATH=f"{Path.home()}/.nargo/bin:{Path.home()}/.bb:{os.environ.get('PATH', '')}")


def field(x):
    return int(x) % P


def hexf(x):
    return f"0x{field(x):064x}"


def h(*vals):
    """Poseidon2 of 1 to 3 fields, by the circuit's own code (circuits/hash_cli)."""
    assert 1 <= len(vals) <= 3
    key = ",".join(str(field(v)) for v in vals)
    cache = _hash_cache()
    if key in cache:
        return cache[key]
    vals3 = list(vals) + [0] * (3 - len(vals))
    prover = CIRCUITS / "hash_cli" / "Prover.toml"
    prover.write_text(f'a = "{field(vals3[0])}"\nb = "{field(vals3[1])}"\nc = "{field(vals3[2])}"\nn = "{len(vals)}"\n')
    out = subprocess.run(["nargo", "execute", "--silence-warnings"], cwd=CIRCUITS / "hash_cli",
                         capture_output=True, text=True, env=ENV)
    m = re.search(r"Circuit output: (0x[0-9a-fA-F]+)", out.stdout + out.stderr)
    if not m:
        raise SystemExit(f"hash_cli failed:\n{out.stdout}\n{out.stderr}")
    v = int(m.group(1), 16)
    cache[key] = v
    _save_hash_cache(cache)
    return v


_HC = None
def _hash_cache():
    global _HC
    if _HC is None:
        f = HERE / "hashes.json"
        _HC = json.loads(f.read_text()) if f.exists() else {}
    return _HC


def _save_hash_cache(c):
    (HERE / "hashes.json").write_text(json.dumps(c))


def zeros():
    """Z[0] = 0 (an empty leaf), Z[i] = H(Z[i-1], Z[i-1]); Z[DEPTH] is the empty tree's root."""
    z = [0]
    for _ in range(DEPTH):
        z.append(h(z[-1], z[-1]))
    return z


def sk_of(owner):
    return int.from_bytes(hashlib.sha256(f"jam-privacy-demo:{owner}".encode()).digest(), "big") % P


def pk_of(owner):
    return h(sk_of(owner))


def load():
    if STATE.exists():
        return json.loads(STATE.read_text())
    return {"notes": [], "leaves": [], "null_root": 0}


def save(st):
    STATE.write_text(json.dumps(st, indent=1))


def note_hash(n):
    return h(pk_of(n["owner"]), n["value"], n["blinding"])


class Tree:
    """An append-only Merkle tree of depth DEPTH over the known leaves; empty leaves are 0."""
    def __init__(self, leaves):
        self.leaves = list(leaves)
        self.z = zeros()

    def node(self, level, index):
        if level == 0:
            return self.leaves[index] if index < len(self.leaves) else 0
        # a subtree with no leaves is the zero hash of its height
        if index * (1 << level) >= len(self.leaves):
            return self.z[level]
        return h(self.node(level - 1, 2 * index), self.node(level - 1, 2 * index + 1))

    def root(self):
        return self.node(DEPTH, 0)

    def path(self, index, from_level=0):
        """Siblings of the node at (from_level, index >> from_level) up to the root."""
        sib = []
        i = index >> from_level
        for level in range(from_level, DEPTH):
            sib.append(self.node(level, i ^ 1))
            i >>= 1
        return sib


def new_blinding(st):
    return int.from_bytes(hashlib.sha256(f"blinding:{len(st['notes'])}:{os.urandom(8).hex()}".encode()).digest(), "big") % P


def cmd_pk(owner):
    print(hexf(pk_of(owner)))


def cmd_deposit(owner, value):
    st = load()
    n = {"id": len(st["notes"]), "owner": owner, "value": int(value), "blinding": new_blinding(st),
         "kind": "deposit", "spent": False, "leaf": None}
    n["hash"] = note_hash(n)
    st["notes"].append(n); save(st)
    print(f"note {n['id']} commitment {hexf(n['hash'])}")


def toml_note_in(n, tree, sender, st):
    if n is None:
        # a dummy input: no value, no membership; the sender's key and a fresh blinding give it
        # a nullifier of its own, since the chain records every nullifier a proof names
        return {"owner_sk": str(sk_of(sender)), "value": "0", "blinding": str(new_blinding(st)), "kind": "0",
                "index": "0", "path": ["0"] * DEPTH}
    kind = "2" if n["kind"] == "deposit" else "1"
    path = tree.path(n["leaf"]) if kind == "1" else [0] * DEPTH
    return {"owner_sk": str(sk_of(n["owner"])), "value": str(n["value"]), "blinding": str(n["blinding"]),
            "kind": kind, "index": str(n["leaf"] or 0), "path": [str(x) for x in path]}


def toml_note_out(n):
    return {"owner_pk": str(pk_of(n["owner"])), "value": str(n["value"]), "blinding": str(n["blinding"])}


def toml_table(name, d):
    lines = [f"[[{name}]]"]
    for k, v in d.items():
        lines.append(f'{k} = {json.dumps(v)}' if isinstance(v, list) else f'{k} = "{v}"')
    return "\n".join(lines)


def pick(st, ref):
    if ref == "-":
        return None
    if ref.startswith("deposit:"):
        ref = ref.split(":", 1)[1]
    n = st["notes"][int(ref)]
    assert not n["spent"], f"note {ref} is spent"
    return n


def cmd_transfer(sender, in_a, in_b, to, value, unshield, receiver, dest, out):
    st = load()
    tree = Tree(st["leaves"])
    old_root, old_null = tree.root(), st["null_root"]
    ins = [pick(st, in_a), pick(st, in_b)]
    total_in = sum(n["value"] for n in ins if n)
    value, unshield = int(value), int(unshield)
    change = total_in - value - unshield
    assert change >= 0, "not enough in the inputs"
    outs = [{"id": len(st["notes"]), "owner": to, "value": value, "blinding": new_blinding(st), "kind": "tree", "spent": False},
            {"id": len(st["notes"]) + 1, "owner": sender, "value": change, "blinding": new_blinding(st), "kind": "tree", "spent": False}]
    insert = len(st["leaves"]); assert insert % 2 == 0
    insert_path = tree.path(insert, from_level=1)
    prover = "\n".join([
        f'old_note_root = "{old_root}"', f'old_null_root = "{old_null}"',
        f'insert_index1 = "{insert // 2}"', f'insert_path = {json.dumps([str(x) for x in insert_path])}',
        f'unshield_amount = "{unshield}"', f'unshield_receiver = "{int(receiver)}"', f'unshield_dest = "{int(dest)}"',
        toml_table("ins", toml_note_in(ins[0], tree, sender, st)), toml_table("ins", toml_note_in(ins[1], tree, sender, st)),
        toml_table("outs", toml_note_out(outs[0])), toml_table("outs", toml_note_out(outs[1])), ""])
    cdir = CIRCUITS / "transfer"
    (cdir / "Prover.toml").write_text(prover)
    r = subprocess.run(["nargo", "execute", "--silence-warnings"], cwd=cdir, capture_output=True, text=True, env=ENV)
    if r.returncode != 0:
        raise SystemExit(f"nargo execute failed:\n{r.stdout}\n{r.stderr}")
    pdir = cdir / "target" / "proof"; pdir.mkdir(parents=True, exist_ok=True)
    for f in ("proof", "public_inputs", "vk", "vk_hash"):
        (pdir / f).unlink(missing_ok=True)
    r = subprocess.run(["bb", "prove", "-s", "ultra_honk", "--oracle_hash", "keccak", "-b", "target/transfer.json",
                        "-w", "target/transfer.gz", "-o", "target/proof", "--write_vk"], cwd=cdir, capture_output=True, text=True, env=ENV)
    if r.returncode != 0:
        raise SystemExit(f"bb prove failed:\n{r.stdout}\n{r.stderr}")
    pubs = (pdir / "public_inputs").read_bytes(); proof = (pdir / "proof").read_bytes()
    assert len(pubs) == 12 * 32, len(pubs)
    payload = len(pubs).to_bytes(2, "little") + pubs + proof
    Path(out).write_text(payload.hex())
    # the client's view of the chain after this package lands
    fields = [int.from_bytes(pubs[i * 32:(i + 1) * 32], "big") for i in range(12)]
    for n in ins:
        if n:
            n["spent"] = True
    for i, o in enumerate(outs):
        o["hash"] = fields[6 + i]; o["leaf"] = insert + i
        st["notes"].append(o); st["leaves"].append(o["hash"])
    st["null_root"] = fields[3]
    save(st)
    print(f"payload {len(payload)} bytes -> {out}")
    print(f"old_note_root {hexf(fields[0])}\nnew_note_root {hexf(fields[2])}\nnew_null_root {hexf(fields[3])}\nnullifiers {hexf(fields[4])} {hexf(fields[5])}")
    print(f"note_hashes {hexf(fields[6])} {hexf(fields[7])}\nunshield {fields[8]} -> account {fields[9]} on service {fields[10]}; deposit {fields[11]}")


def cmd_notes(owner=None):
    for n in load()["notes"]:
        if not n["spent"] and (owner is None or n["owner"] == owner):
            print(f"note {n['id']}: {n['owner']} {n['value']} ({n['kind']}, leaf {n['leaf']})")


def cmd_root():
    print(hexf(Tree(load()["leaves"]).root()))


def cmd_sync(path):
    st = load(); d = json.loads(Path(path).read_text())
    st["leaves"] = [int(x, 16) for x in d["leaves"]]
    st["null_root"] = int(d["null_root"], 16) if d.get("null_root") else 0
    for n in st["notes"]:              # a note's leaf is wherever the chain put it
        if n.get("hash") in st["leaves"]:
            n["leaf"] = st["leaves"].index(n["hash"])
    save(st); print(f"synced {len(st['leaves'])} leaves, root {hexf(Tree(st['leaves']).root())}")


if __name__ == "__main__":
    a = sys.argv[1:]
    {"pk": cmd_pk, "deposit": cmd_deposit, "transfer": cmd_transfer, "notes": cmd_notes, "root": cmd_root, "sync": cmd_sync}[a[0]](*a[1:])
