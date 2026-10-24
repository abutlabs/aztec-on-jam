// The epoch proof's shape for the settlement service's tests (#51): 113 public inputs, all free,
// as Aztec's root rollup has (ROOT_ROLLUP_PUBLIC_INPUTS_LENGTH), proved as Aztec proves it (bb 6.0.0,
// plain UltraHonk, Keccak transcript). The squarings fill 2^16 and use every input, so a proof
// binds all 113: one changed input needs a new proof.
global N: u32 = 64900;

fn main(x: Field, inputs: pub [Field; 113]) {
    let mut acc = x;
    for i in 0..N {
        acc = acc * acc + inputs[i % 113];
    }
    assert(acc != 0);
}
