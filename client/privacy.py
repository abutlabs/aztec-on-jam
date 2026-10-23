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
  privacy.py scan <owner> <state.json>                find the owner's notes in the chain's sealed
      notes ({"leaves": [...], "sealed": [...]} from the indexer's /state), and mark spent the
      ones whose nullifiers the chain has (<state.json>'s "spent" list, if given)
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
# the pinned toolchain (docs/TOOLS.md; scripts/bootstrap.sh installs it), overridable
NOIR_VERSION, BB_VERSION = "1.0.0-rc.5", "5.0.0"


def _tool(env, versioned, plain):
    if os.environ.get(env):
        return os.environ[env]
    return str(versioned) if versioned.exists() else plain


NARGO = _tool("NARGO", Path.home() / ".nargo" / "versions" / f"nargo-{NOIR_VERSION}", "nargo")
BB = _tool("BB", Path.home() / ".bb" / "versions" / f"bb-{BB_VERSION}", "bb")


_WORK = {}


def circuit(name):
    """This process's own copy of circuits/<name> (Nargo.toml, src, and the compiled program
    if it is current), made on first use and removed at exit. Proving writes Prover.toml, the
    witness and the proof inside it, so two clients (the wallet, the soak, check) never
    overwrite each other's files, and a long run keeps the circuit it started with even if
    the source changes under it."""
    if name not in _WORK:
        import atexit, shutil
        work = Path(os.environ.get("CLIENT_WORK") or ROOT / "out" / "work") / f"{os.getpid()}" / name
        if work.exists():
            shutil.rmtree(work)
        src = CIRCUITS / name
        work.mkdir(parents=True)
        shutil.copy(src / "Nargo.toml", work / "Nargo.toml")
        shutil.copytree(src / "src", work / "src")
        art = src / "target" / f"{name}.json"
        newest_src = max(f.stat().st_mtime for f in (src / "src").rglob("*") if f.is_file())
        if art.exists() and art.stat().st_mtime >= newest_src:
            (work / "target").mkdir()
            shutil.copy(art, work / "target" / art.name)
        atexit.register(shutil.rmtree, work.parent, True)
        _WORK[name] = work
    return _WORK[name]


class ClientError(Exception):
    """A proof, a hash or an input selection failed; the message says which and why."""


def field(x):
    return int(x) % P


def hexf(x):
    return f"0x{field(x):064x}"


def h(*vals):
    """Poseidon2 of 1 to 3 fields: noir-lang/poseidon's `Poseidon2::hash`, the circuit's own
    hash, computed in-process (client/poseidon2.py; tested against circuits/hash_cli)."""
    assert 1 <= len(vals) <= 3
    import poseidon2
    return poseidon2.hash([field(v) for v in vals])


def seed_path(n0, n1, index1, siblings):
    """The new root when two leaves fill the level-1 slot `index1` with these siblings: the
    same computation as circuits/path_cli, in-process."""
    node = h(n0, n1)
    for i, sib in enumerate(siblings):
        node = h(sib, node) if (int(index1) >> i) & 1 else h(node, sib)
    return node


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
        self.memo = {}

    def node(self, level, index):
        if level == 0:
            return self.leaves[index] if index < len(self.leaves) else 0
        # a subtree with no leaves is the zero hash of its height
        if index * (1 << level) >= len(self.leaves):
            return self.z[level]
        if (level, index) not in self.memo:
            self.memo[(level, index)] = h(self.node(level - 1, 2 * index), self.node(level - 1, 2 * index + 1))
        return self.memo[(level, index)]

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


def deposit(owner, value):
    """A new deposit note for a shield of `value` to `owner`; returns the note (its hash is C)."""
    st = load()
    n = {"id": len(st["notes"]), "owner": owner, "value": int(value), "blinding": new_blinding(st),
         "kind": "deposit", "spent": False, "leaf": None}
    n["hash"] = note_hash(n)
    st["notes"].append(n); save(st)
    return n


def cmd_deposit(owner, value):
    n = deposit(owner, value)
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
    return {"owner_pk": str(owner_pk(n["owner"])), "value": str(n["value"]), "blinding": str(n["blinding"])}


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
    if n["spent"]:
        raise ClientError(f"note {ref} is spent")
    return n


def transfer_vk():
    """The transfer circuit's verification key for bb's evm target (zk, Keccak), written once
    per circuit build. The service embeds the same key (services/privacy/fixtures)."""
    cdir = circuit("transfer"); art = cdir / "target" / "transfer.json"; vdir = cdir / "target" / "vk"
    if not art.exists():
        r = subprocess.run([NARGO, "compile", "--silence-warnings"], cwd=cdir, capture_output=True, text=True, env=ENV)
        if r.returncode != 0:
            raise ClientError(f"nargo compile failed:\n{r.stdout}\n{r.stderr}")
    vk = vdir / "vk"
    if not vk.exists() or vk.stat().st_mtime < art.stat().st_mtime:
        vdir.mkdir(parents=True, exist_ok=True)
        r = subprocess.run([BB, "write_vk", "-t", "evm", "-b", str(art), "-o", str(vdir)], capture_output=True, text=True, env=ENV)
        if r.returncode != 0:
            raise ClientError(f"bb write_vk failed:\n{r.stdout}\n{r.stderr}")
    return vk


def enc_keys(owner):
    """(secret, public) X25519 note-encryption keys of a named user (the Python client's
    users are names; the browser wallet keeps a random key instead)."""
    import notecrypt
    return notecrypt.keypair(owner.encode().ljust(32, b"\0")[:32])


def _directory(owner):
    """The owner's directory entry, 64 bytes (encryption key || spend public key), from
    PRIVACY_KEYS (a JSON map of owner -> hex the caller read from the chain), or None."""
    path = os.environ.get("PRIVACY_KEYS")
    if path and Path(path).exists():
        k = json.loads(Path(path).read_text()).get(owner)
        if k and len(k) == 128:
            return bytes.fromhex(k)
    return None


def directory_key(owner):
    """The owner's note-encryption key: the directory's if it names the owner, else derived."""
    d = _directory(owner)
    return d[:32] if d else enc_keys(owner)[1]


def owner_pk(owner):
    """The owner's spend public key: the directory's if it names the owner, else derived."""
    d = _directory(owner)
    return int.from_bytes(d[32:], "big") if d else pk_of(owner)


def directory_entry(owner):
    """What `owner` publishes in the directory: encryption key || spend public key, hex."""
    return enc_keys(owner)[1].hex() + f"{pk_of(owner):064x}"


def seal_notes(outs, enc_pk_of):
    """The two output notes' openings, each sealed to its owner: the logs the proof commits
    to. Returns (logs bytes, logs_hash as a field element)."""
    import notecrypt
    logs = b"".join(notecrypt.seal(enc_pk_of(o["owner"]), notecrypt.note_plaintext(o["value"], o["blinding"])) for o in outs)
    h = bytearray(hashlib.blake2b(logs, digest_size=32).digest()); h[0] = 0
    return logs, int.from_bytes(h, "big")


def transfer(sender, in_a, in_b, to, value, unshield, receiver, dest, enc_pk_of=None):
    """Prove a join-split; returns {"payload": bytes, "fields": [12 ints]} and records the
    client's view of the chain after it lands (inputs spent, outputs as leaves)."""
    st = load()
    tree = Tree(st["leaves"])
    old_root, old_null = tree.root(), st["null_root"]
    ins = [pick(st, in_a), pick(st, in_b)]
    total_in = sum(n["value"] for n in ins if n)
    value, unshield = int(value), int(unshield)
    change = total_in - value - unshield
    if change < 0:
        raise ClientError(f"not enough in the inputs: {total_in} < {value} + {unshield}")
    outs = [{"id": len(st["notes"]), "owner": to, "value": value, "blinding": new_blinding(st), "kind": "tree", "spent": False},
            {"id": len(st["notes"]) + 1, "owner": sender, "value": change, "blinding": new_blinding(st), "kind": "tree", "spent": False}]
    insert = len(st["leaves"])
    if insert % 2:
        raise ClientError(f"the tree has an odd number of leaves ({insert}); sync first")
    insert_path = tree.path(insert, from_level=1)
    logs, logs_hash = seal_notes(outs, enc_pk_of or directory_key)
    prover = "\n".join([
        f'old_note_root = "{old_root}"', f'old_null_root = "{old_null}"', f'logs_hash = "{logs_hash}"',
        f'insert_index1 = "{insert // 2}"', f'insert_path = {json.dumps([str(x) for x in insert_path])}',
        f'unshield_amount = "{unshield}"', f'unshield_receiver = "{int(receiver)}"', f'unshield_dest = "{int(dest)}"',
        toml_table("ins", toml_note_in(ins[0], tree, sender, st)), toml_table("ins", toml_note_in(ins[1], tree, sender, st)),
        toml_table("outs", toml_note_out(outs[0])), toml_table("outs", toml_note_out(outs[1])), ""])
    cdir = circuit("transfer")
    (cdir / "Prover.toml").write_text(prover)
    r = subprocess.run([NARGO, "execute", "--silence-warnings"], cwd=cdir, capture_output=True, text=True, env=ENV)
    if r.returncode != 0:
        raise ClientError(f"nargo execute failed:\n{r.stdout}\n{r.stderr}")
    pdir = cdir / "target" / "proof"; pdir.mkdir(parents=True, exist_ok=True)
    for f in ("proof", "public_inputs", "vk", "vk_hash"):
        (pdir / f).unlink(missing_ok=True)
    vk = transfer_vk()
    r = subprocess.run([BB, "prove", "-t", "evm", "-b", "target/transfer.json", "-w", "target/transfer.gz",
                        "-k", str(vk), "-o", "target/proof"], cwd=cdir, capture_output=True, text=True, env=ENV)
    if r.returncode != 0:
        raise ClientError(f"bb prove failed:\n{r.stdout}\n{r.stderr}")
    pubs = (pdir / "public_inputs").read_bytes(); proof = (pdir / "proof").read_bytes()
    if len(pubs) != 13 * 32:
        raise ClientError(f"expected 13 public inputs, got {len(pubs) // 32}")
    # [pubs_len u16][pubs][proof_len u16][proof][the two sealed notes]
    payload = len(pubs).to_bytes(2, "little") + pubs + len(proof).to_bytes(2, "little") + proof + logs
    # the client's view of the chain after this package lands
    # the 12-field view callers use: every public input but logs_hash (index 2)
    all13 = [int.from_bytes(pubs[i * 32:(i + 1) * 32], "big") for i in range(13)]
    fields = all13[:2] + all13[3:]
    for n in ins:
        if n:
            n["spent"] = True
    for i, o in enumerate(outs):
        o["hash"] = fields[6 + i]; o["leaf"] = insert + i
        st["notes"].append(o); st["leaves"].append(o["hash"])
    st["null_root"] = fields[3]
    save(st)
    # the new nodes along the insertion path, in one execution, checked against the proof's root
    if seed_path(fields[6], fields[7], insert // 2, insert_path) != fields[2]:
        raise ClientError("path_cli's root differs from the proof's new root")
    return {"payload": payload, "fields": fields, "logs": logs, "logs_hash": all13[2]}


def cmd_transfer(sender, in_a, in_b, to, value, unshield, receiver, dest, out):
    r = transfer(sender, in_a, in_b, to, value, unshield, receiver, dest)
    payload, fields = r["payload"], r["fields"]
    Path(out).write_text(payload.hex())
    print(f"payload {len(payload)} bytes -> {out}")
    print(f"old_note_root {hexf(fields[0])}\nnew_note_root {hexf(fields[2])}\nold_null_root {hexf(fields[1])}\nnew_null_root {hexf(fields[3])}\nnullifiers {hexf(fields[4])} {hexf(fields[5])}")
    print(f"note_hashes {hexf(fields[6])} {hexf(fields[7])}\nunshield {fields[8]} -> account {fields[9]} on service {fields[10]}; deposit {fields[11]}")


def cmd_notes(owner=None):
    for n in load()["notes"]:
        if not n["spent"] and (owner is None or n["owner"] == owner):
            print(f"note {n['id']}: {n['owner']} {n['value']} ({n['kind']}, leaf {n['leaf']})")


def cmd_root():
    print(hexf(Tree(load()["leaves"]).root()))


def sync(d):
    """Take the chain's leaves and nullifier root ({"leaves": [hex], "null_root": hex})."""
    st = load()
    st["leaves"] = [int(x, 16) for x in d["leaves"]]
    st["null_root"] = int(d["null_root"], 16) if d.get("null_root") else 0
    for n in st["notes"]:              # a note's leaf is wherever the chain put it
        if n.get("hash") in st["leaves"]:
            n["leaf"] = st["leaves"].index(n["hash"])
    save(st)
    return len(st["leaves"])


def scan(owner, chain_state, is_spent=None):
    """Open every sealed note on chain this client has not seen with the owner's key; each
    that opens and hashes to its leaf is the owner's. A receiver needs nothing but the chain
    and its key. Returns the notes found."""
    import notecrypt
    sync(chain_state)
    st = load()
    enc_sk = enc_keys(owner)[0]
    known = {n["leaf"] for n in st["notes"] if n.get("leaf") is not None}
    pk = pk_of(owner)
    found = []
    for i, (leaf_hex, sealed_hex) in enumerate(zip(chain_state.get("leaves", []), chain_state.get("sealed", []))):
        if i in known or not sealed_hex:
            continue
        pt = notecrypt.open_(enc_sk, bytes.fromhex(sealed_hex))
        if pt is None:
            continue
        value, blinding = notecrypt.note_from_plaintext(pt)
        nh = h(pk, value, blinding)
        if nh != int(leaf_hex, 16):
            continue                   # opens, but not the note at this leaf: not trusted
        n = {"id": len(st["notes"]), "owner": owner, "value": value, "blinding": blinding,
             "kind": "tree", "spent": False, "leaf": i, "hash": nh}
        if is_spent is not None and is_spent(f"{h(nh, sk_of(owner)):064x}"):
            n["spent"] = True
        st["notes"].append(n); found.append(n)
    save(st)
    return found


def cmd_scan(owner, path):
    d = json.loads(Path(path).read_text())
    spent = set(d.get("spent", []))
    for n in scan(owner, d, (lambda nf: nf in spent) if "spent" in d else None):
        print(f"found note {n['id']}: {owner} {n['value']} at leaf {n['leaf']}{' (spent)' if n['spent'] else ''}")


def cmd_sync(path):
    n = sync(json.loads(Path(path).read_text()))
    print(f"synced {n} leaves, root {hexf(Tree(load()['leaves']).root())}")


def unspent(owner):
    """The owner's spendable notes: in the tree with a leaf, or an unclaimed deposit."""
    return [n for n in load()["notes"] if n["owner"] == owner and not n["spent"] and n["value"] > 0
            and (n["kind"] == "deposit" or n.get("leaf") is not None)]


def private_balance(owner):
    return sum(n["value"] for n in unspent(owner))


def select_inputs(owner, amount):
    """Two input refs covering `amount` from the owner's notes, largest first; a deposit goes
    in the first slot (the circuit allows a deposit only there)."""
    notes = sorted(unspent(owner), key=lambda n: (n["kind"] != "deposit", -n["value"]))
    for k in (1, 2):
        for i in range(len(notes)):
            pick_ = notes[i:i + k]
            if len(pick_) == k and sum(n["value"] for n in pick_) >= amount:
                if k == 2 and pick_[1]["kind"] == "deposit":
                    continue
                refs = [("deposit:%d" if n["kind"] == "deposit" else "%d") % n["id"] for n in pick_]
                return refs + ["-"] * (2 - k)
    raise ClientError(f"{owner} holds {private_balance(owner)} privately, not {amount} in two notes")


if __name__ == "__main__":
    a = sys.argv[1:]
    {"pk": cmd_pk, "deposit": cmd_deposit, "transfer": cmd_transfer, "notes": cmd_notes, "root": cmd_root, "sync": cmd_sync, "scan": cmd_scan, "enckey": lambda o: print(enc_keys(o)[1].hex()), "directory": lambda o: print(directory_entry(o))}[a[0]](*a[1:])
