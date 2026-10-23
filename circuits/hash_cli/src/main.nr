// The client's hash oracle: Poseidon2 of the first n of three fields, the same function the
// transfer circuit uses, so the client's notes, nullifiers and tree agree with the proof.
use poseidon::poseidon2::Poseidon2;

fn main(a: Field, b: Field, c: Field, n: u32) -> pub Field {
    if n == 1 {
        Poseidon2::hash([a], 1)
    } else if n == 2 {
        Poseidon2::hash([a, b], 2)
    } else {
        Poseidon2::hash([a, b, c], 3)
    }
}
