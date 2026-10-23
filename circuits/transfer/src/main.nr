// A private transfer, join-split: two notes in, two notes out, an optional unshield to a
// public account and an optional deposit claim. Aztec's shapes (note hash, nullifier, an
// append-only note tree) with Poseidon2; the nullifier set's non-membership is the chain's
// (the service refuses a spent nullifier), the nullifier "root" here is a running hash of
// what this proof spends. Public inputs, in order (params first, then the returned tuple):
//   0 old_note_root  1 old_null_root            (params)
//   2 new_note_root  3 new_null_root  4,5 nullifiers  6,7 note_hashes
//   8 unshield_amount  9 unshield_receiver  10 unshield_dest  11 deposit_amount  (returns)
use poseidon::poseidon2::Poseidon2;

global DEPTH: u32 = 16;
global KIND_DUMMY: Field = 0;
global KIND_TREE: Field = 1;
global KIND_DEPOSIT: Field = 2;

struct InNote {
    owner_sk: Field,
    value: Field,
    blinding: Field,
    kind: Field, // 0 dummy (value 0, no membership), 1 in the tree, 2 a shield deposit
    index: Field, // leaf index, kind 1
    path: [Field; DEPTH], // siblings, leaf level first, kind 1
}

struct OutNote {
    owner_pk: Field,
    value: Field,
    blinding: Field,
}

fn pk_of(sk: Field) -> Field {
    Poseidon2::hash([sk], 1)
}

fn note_hash(owner_pk: Field, value: Field, blinding: Field) -> Field {
    Poseidon2::hash([owner_pk, value, blinding], 3)
}

fn nullifier_of(note_hash: Field, owner_sk: Field) -> Field {
    Poseidon2::hash([note_hash, owner_sk], 2)
}

fn hash2(a: Field, b: Field) -> Field {
    Poseidon2::hash([a, b], 2)
}

// The root of a depth-DEPTH tree from a leaf, its index and the siblings from the leaf up.
fn merkle_root(leaf: Field, index: Field, path: [Field; DEPTH]) -> Field {
    let bits: [bool; DEPTH] = index.to_le_bits();
    let mut node = leaf;
    for i in 0..DEPTH {
        let (l, r) = if bits[i] { (path[i], node) } else { (node, path[i]) };
        node = hash2(l, r);
    }
    node
}

// The root from a level-1 node (the parent of two leaves), its index at level 1 and the
// siblings from level 1 up.
fn root_from_level1(node1: Field, index1: Field, path: [Field; DEPTH - 1]) -> Field {
    let bits: [bool; DEPTH - 1] = index1.to_le_bits();
    let mut node = node1;
    for i in 0..DEPTH - 1 {
        let (l, r) = if bits[i] { (path[i], node) } else { (node, path[i]) };
        node = hash2(l, r);
    }
    node
}

fn as_u64(v: Field) -> u64 {
    let x = v as u64;
    assert(x as Field == v, "value exceeds u64");
    x
}

fn main(
    old_note_root: pub Field,
    old_null_root: pub Field,
    ins: [InNote; 2],
    outs: [OutNote; 2],
    insert_index1: Field, // the level-1 slot the two new leaves fill (leaf indices 2k, 2k+1)
    insert_path: [Field; DEPTH - 1], // siblings of that slot from level 1 up, under the old root
    unshield_amount: Field,
    unshield_receiver: Field,
    unshield_dest: Field,
) -> pub (Field, Field, [Field; 2], [Field; 2], Field, Field, Field, Field) {
    let mut nullifiers = [0; 2];
    let mut total_in: u64 = 0;
    let mut deposit_amount: u64 = 0;
    for i in 0..2 {
        let n = ins[i];
        let nh = note_hash(pk_of(n.owner_sk), n.value, n.blinding);
        let v = as_u64(n.value);
        assert((n.kind == KIND_DUMMY) | (n.kind == KIND_TREE) | (n.kind == KIND_DEPOSIT));
        if n.kind == KIND_DUMMY {
            assert(v == 0, "a dummy note has no value");
        }
        if n.kind == KIND_TREE {
            assert(merkle_root(nh, n.index, n.path) == old_note_root, "note not in the tree");
        }
        if n.kind == KIND_DEPOSIT {
            assert(i == 0, "only the first input may be a deposit");
            deposit_amount += v;
        }
        // a deposit's nullifier is its commitment: the chain matches it to the shield's memo
        nullifiers[i] = if n.kind == KIND_DEPOSIT { nh } else { nullifier_of(nh, n.owner_sk) };
        total_in += v;
    }
    assert(nullifiers[0] != nullifiers[1], "the two inputs are the same note");

    let mut note_hashes = [0; 2];
    let mut total_out: u64 = 0;
    for i in 0..2 {
        note_hashes[i] = note_hash(outs[i].owner_pk, outs[i].value, outs[i].blinding);
        total_out += as_u64(outs[i].value);
    }
    let unshield = as_u64(unshield_amount);
    assert(total_in == total_out + unshield, "the transfer does not balance");

    // the slot must be empty under the old root, then holds the two new leaves
    let empty1 = hash2(0, 0);
    assert(root_from_level1(empty1, insert_index1, insert_path) == old_note_root, "slot not empty");
    let new_note_root = root_from_level1(hash2(note_hashes[0], note_hashes[1]), insert_index1, insert_path);
    let new_null_root = hash2(hash2(old_null_root, nullifiers[0]), nullifiers[1]);

    (new_note_root, new_null_root, nullifiers, note_hashes, unshield_amount, unshield_receiver, unshield_dest, deposit_amount as Field)
}
