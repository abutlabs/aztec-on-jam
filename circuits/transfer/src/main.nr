// A private transfer, join-split: two notes in, two notes out, an optional unshield to a
// public account and an optional deposit claim. Aztec's shapes (note hash, nullifier, an
// append-only note tree) with Poseidon2; the nullifier set's non-membership is the chain's
// (the service refuses a spent nullifier), the nullifier "root" here is a running hash of
// what this proof spends. Public inputs, in order (params first, then the returned tuple):
//   0 old_note_root  1 old_null_root  2 logs_hash  3 old_null_size        (params)
//   4 new_note_root  5 new_null_root  6,7 nullifiers  8,9 note_hashes
//   10 unshield_amount  11 unshield_receiver  12 unshield_dest  13 deposit_amount  (returns)
// The nullifier tree is Aztec's indexed Merkle tree: leaf = H(value, next_index, next_value),
// a sorted linked list (leaf 0 is the zero leaf); a nullifier is proven absent by the leaf
// that brackets it, then inserted at index old_null_size + i. The chain checks the old root
// and size, so non-membership is proven here, not looked up by the service.
// logs_hash binds the two sealed notes (the openings, encrypted to their owners) to the
// proof: the service checks blake2b(sealed notes), first byte zeroed, against it, so no one
// can swap the ciphertexts a receiver will scan for.
use poseidon::poseidon2::Poseidon2;

global DEPTH: u32 = 16;      // the note tree
global NULL_DEPTH: u32 = 16; // the nullifier tree
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

// The nullifier-tree leaf that brackets a new nullifier: value < nullifier < next_value
// (or next_value is 0, the end of the list), with its index and path.
struct LowLeaf {
    value: Field,
    next_index: Field,
    next_value: Field,
    index: Field,
    path: [Field; NULL_DEPTH],
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

fn root_n(leaf: Field, index: Field, path: [Field; NULL_DEPTH]) -> Field {
    let bits: [bool; NULL_DEPTH] = index.to_le_bits();
    let mut node = leaf;
    for i in 0..NULL_DEPTH {
        let (l, r) = if bits[i] { (path[i], node) } else { (node, path[i]) };
        node = hash2(l, r);
    }
    node
}

fn null_leaf(value: Field, next_index: Field, next_value: Field) -> Field {
    Poseidon2::hash([value, next_index, next_value], 3)
}

/// Insert `nf` into the indexed nullifier tree with root `root` at `new_index`, proving it was
/// absent (the low leaf brackets it); returns the new root.
fn insert_nullifier(root: Field, nf: Field, new_index: Field, low: LowLeaf, slot_path: [Field; NULL_DEPTH]) -> Field {
    assert(root_n(null_leaf(low.value, low.next_index, low.next_value), low.index, low.path) == root, "low leaf not in the nullifier tree");
    assert(low.value.lt(nf), "the low leaf is not below the nullifier");
    assert((low.next_value == 0) | nf.lt(low.next_value), "the nullifier is already spent (or the low leaf is wrong)");
    // the low leaf now points at the new one
    let root = root_n(null_leaf(low.value, new_index, nf), low.index, low.path);
    // the new leaf goes into an empty slot and takes over the low leaf's old pointer
    assert(root_n(0, new_index, slot_path) == root, "the nullifier slot is not empty");
    root_n(null_leaf(nf, low.next_index, low.next_value), new_index, slot_path)
}

fn as_u64(v: Field) -> u64 {
    let x = v as u64;
    assert(x as Field == v, "value exceeds u64");
    x
}

fn main(
    old_note_root: pub Field,
    old_null_root: pub Field,
    logs_hash: pub Field,
    old_null_size: pub Field, // the nullifier tree's next free index
    ins: [InNote; 2],
    lows: [LowLeaf; 2], // per nullifier, the leaf that brackets it (the second against the tree after the first)
    null_slot_paths: [[Field; NULL_DEPTH]; 2], // per nullifier, its empty slot's siblings
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
    let mut new_null_root = old_null_root;
    for i in 0..2 {
        new_null_root = insert_nullifier(new_null_root, nullifiers[i], old_null_size + i as Field, lows[i], null_slot_paths[i]);
    }

    (new_note_root, new_null_root, nullifiers, note_hashes, unshield_amount, unshield_receiver, unshield_dest, deposit_amount as Field)
}
