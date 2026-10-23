// Step-1 stub: the public-input SHAPE of a private transfer, so the service's refine and
// accumulate can be built and checked against real UltraHonk proofs. The constraints that
// make it a transfer (Merkle membership of the spent notes, nullifier derivation, balance,
// the new roots) come in step 2; until then this proves only that the prover knew a
// secret whose hash is bound into the first nullifier (pedersen for the stub; Aztec's trees use Poseidon2, step 2). Decision issue: #16.
fn main(
    secret: Field,
    old_note_root: pub Field,
    old_null_root: pub Field,
    new_note_root: pub Field,
    new_null_root: pub Field,
    nullifiers: pub [Field; 2],
    note_hashes: pub [Field; 2],
    // unshield: amount (0 = none) to a public account on the public-balance service
    unshield_amount: pub Field,
    unshield_receiver: pub Field,
    unshield_dest: pub Field,
) {
    let bound = std::hash::pedersen_hash([secret, old_note_root]);
    assert(nullifiers[0] == bound);
    assert(new_note_root != old_note_root);
    assert(new_null_root != old_null_root);
    assert(note_hashes[0] != note_hashes[1]);
}

#[test]
fn print_binding() {
    let bound = std::hash::pedersen_hash([7, 0]);
    std::println(bound);
}
