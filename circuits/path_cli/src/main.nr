// The client's tree oracle: the nodes from level 1 to the root when two new leaves fill the
// level-1 slot `index1`, given that slot's siblings from level 1 up. One execution instead of
// one per hash; the same Poseidon2 and the same left/right rule as circuits/transfer.
use poseidon::poseidon2::Poseidon2;

global DEPTH: u32 = 16;

fn hash2(a: Field, b: Field) -> Field {
    Poseidon2::hash([a, b], 2)
}

fn main(n0: Field, n1: Field, index1: Field, siblings: [Field; DEPTH - 1]) -> pub [Field; DEPTH] {
    let bits: [bool; DEPTH - 1] = index1.to_le_bits();
    let mut nodes = [0; DEPTH];
    let mut node = hash2(n0, n1);
    nodes[0] = node;
    for i in 0..DEPTH - 1 {
        let (l, r) = if bits[i] { (siblings[i], node) } else { (node, siblings[i]) };
        node = hash2(l, r);
        nodes[i + 1] = node;
    }
    nodes
}
