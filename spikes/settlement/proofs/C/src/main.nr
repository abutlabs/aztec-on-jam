use std::embedded_curve_ops::{EmbeddedCurvePoint, embedded_curve_add};

global N: u32 = 1;

fn main(x: u64, i: u32, pk_x: [u8; 32], pk_y: [u8; 32], sig: [u8; 64], msg: [u8; 32], inputs: pub [Field; 112]) -> pub Field {
    let mut s: [u64; 25] = [x; 25];
    let mut p: [Field; 4] = [x as Field, 1, 2, 3];
    let g = EmbeddedCurvePoint { x: 1, y: 17631683881184975370165255887551781615748388533673675138860 };
    let mut q = g;
    let mut ram: [Field; 32] = [0; 32];
    let mut acc: Field = 0;
    for k in 0..N {
        s[0] = s[0] ^ (inputs[k % 112] as u64);
        s = std::hash::keccakf1600(s);
        p = std::hash::poseidon2_permutation(p);
        q = embedded_curve_add(q, g);
        let j = (i + k) % 32;
        ram[j] = ram[j] + p[0];
        acc = acc + inputs[(i + k) % 112] + ram[(j + 7) % 32] + ((s[1] as u8) as Field);
    }
    let ok = std::ecdsa_secp256k1::verify_signature(pk_x, pk_y, sig, msg);
    acc + q.x + (ok as Field)
}
