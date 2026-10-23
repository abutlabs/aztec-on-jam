//! Poseidon2 over BN254, t = 4, x^5, 8 full and 56 partial rounds: the permutation Noir's
//! blackbox solver runs, and noir-lang/poseidon v0.4.0's `Poseidon2::hash` sponge (rate 3, the
//! input length times 2^64 as the initial capacity). A line-for-line port of client/poseidon2.py,
//! held to the same vectors. Plus the tree work a privacy service's accumulate would do: a
//! depth-40 root from a leaf and its path.
#![no_std]

mod constants;
use ark_bn254::Fr;
use ark_ff::{AdditiveGroup, Field};
use constants::{DIAG, RC, ROUNDS_F, ROUNDS_P};

fn mds4(s: &mut [Fr; 4]) {
    let t0 = s[0] + s[1];
    let t1 = s[2] + s[3];
    let t2 = s[1].double() + t1;
    let t3 = s[3].double() + t0;
    let t4 = t1.double().double() + t3;
    let t5 = t0.double().double() + t2;
    *s = [t3 + t5, t5, t2 + t4, t4];
}

fn sbox(x: Fr) -> Fr {
    let x2 = x.square();
    x2.square() * x
}

pub fn permute(s: &mut [Fr; 4]) {
    mds4(s);
    for r in 0..ROUNDS_F / 2 {
        for i in 0..4 {
            s[i] = sbox(s[i] + RC[r][i]);
        }
        mds4(s);
    }
    for r in ROUNDS_F / 2..ROUNDS_F / 2 + ROUNDS_P {
        s[0] = sbox(s[0] + RC[r][0]);
        let total = s[0] + s[1] + s[2] + s[3];
        for i in 0..4 {
            s[i] = s[i] * DIAG[i] + total;
        }
    }
    for r in ROUNDS_F / 2 + ROUNDS_P..ROUNDS_F + ROUNDS_P {
        for i in 0..4 {
            s[i] = sbox(s[i] + RC[r][i]);
        }
        mds4(s);
    }
}

/// noir-lang/poseidon `Poseidon2::hash(inputs, inputs.len())`.
pub fn hash(inputs: &[Fr]) -> Fr {
    let n = inputs.len();
    let mut s = [Fr::ZERO, Fr::ZERO, Fr::ZERO, Fr::from(n as u64) * Fr::from(1u128 << 64)];
    for c in 0..n / 3 {
        for i in 0..3 {
            s[i] += inputs[3 * c + i];
        }
        permute(&mut s);
    }
    let rem = n % 3;
    for j in 0..rem {
        s[j] += inputs[n - rem + j];
    }
    if n == 0 || rem != 0 {
        permute(&mut s);
    }
    s[0]
}

/// The root above `leaf` at `index`, from its siblings (one hash a level), as a tree insertion
/// rehashes its path.
pub fn root_from_path(leaf: Fr, index: u64, siblings: &[Fr]) -> Fr {
    let mut node = leaf;
    for (level, sib) in siblings.iter().enumerate() {
        node = if (index >> level) & 1 == 1 { hash(&[*sib, node]) } else { hash(&[node, *sib]) };
    }
    node
}
