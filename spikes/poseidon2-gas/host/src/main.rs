//! p2-host <client/poseidon2-bn254.json>: the permutation vectors, hash_cli's H(7,11,13) and
//! H(0,0), and the empty depth-16 root, as client/poseidon2.py checks them.
use ark_bn254::Fr;
use ark_ff::{BigInteger, PrimeField};
use std::{env, fs};

fn fr(h: &str) -> Fr { Fr::from_be_bytes_mod_order(&num_bigint::BigUint::parse_bytes(h.trim_start_matches("0x").as_bytes(), 16).unwrap().to_bytes_be()) }
fn hex(x: Fr) -> String { format!("0x{}", x.into_bigint().to_bytes_be().iter().map(|b| format!("{b:02x}")).collect::<String>()) }

fn main() {
    let c: serde_json::Value = serde_json::from_str(&fs::read_to_string(env::args().nth(1).expect("json")).unwrap()).unwrap();
    let mut n = 0;
    for v in c["permutation_vectors"].as_array().unwrap() {
        let mut s: [Fr; 4] = core::array::from_fn(|i| fr(v["in"][i].as_str().unwrap()));
        p2::permute(&mut s);
        for i in 0..4 { assert_eq!(s[i], fr(v["out"][i].as_str().unwrap()), "permutation vector {n}"); }
        n += 1;
    }
    let h = p2::hash(&[Fr::from(7u64), Fr::from(11u64), Fr::from(13u64)]);
    assert_eq!(hex(h), "0x1c7467e229cb2aadc6793686c07a90a4b96f2ee7c6b557e3c6ef5d92f1fa253c");
    assert_eq!(hex(p2::hash(&[Fr::from(0u64), Fr::from(0u64)])), "0x0b63a53787021a4a962a452c2921b3663aff1ffd8d5510540f8e659e782956f1");
    let mut z = Fr::from(0u64);
    for _ in 0..16 { z = p2::hash(&[z, z]); }
    assert_eq!(hex(z), "0x14f44d672eb357739e42463497f9fdac46623af863eea4d947ca00a497dcdeb3");
    println!("p2: {n} permutation vectors, hash_cli's H(7,11,13) and H(0,0), and the empty depth-16 root all match");
}
