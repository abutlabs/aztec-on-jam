//! jip-bench <kzg vectors dir> <ecrecover inputs.txt>: the median of many single-threaded runs
//! of each operation the JIP proposes as a host call, natively on this machine. For the three
//! whose PVM gas was measured on PolkaJam with the same Rust code (Poseidon2, the KZG point
//! evaluation, ecrecover), K = PVM gas / native ns: what the PVM charges per nanosecond of
//! native work.
use ark_bn254::{Bn254, Fr as BnFr, G1Affine, G1Projective, G2Affine};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
use ark_std::{rand::SeedableRng, UniformRand};
use std::{env, fs, hint::black_box, time::Instant};

fn median_ns<F: FnMut()>(mut f: F) -> f64 {
    for _ in 0..3 { f(); }
    let mut t = Vec::new();
    let reps = 25;
    for _ in 0..reps {
        let s = Instant::now();
        f();
        t.push(s.elapsed().as_nanos() as f64);
    }
    t.sort_by(|a, b| a.partial_cmp(b).unwrap());
    t[reps / 2]
}

fn unhex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }

fn main() {
    let a: Vec<String> = env::args().collect();
    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(32);
    println!("| operation | native (median of 25, one thread) | PVM gas (PolkaJam, same Rust) | K = gas / ns |");
    println!("|---|---|---|---|");
    // machine-readable too, for pricing.py: "NATIVE <name> <ns>"
    let row = |name: &str, ns: f64, gas: Option<f64>| {
        match gas {
            Some(g) => println!("| {name} | {:.1} µs | {} | {:.1} |", ns / 1e3, g as u64, g / ns),
            None => println!("| {name} | {:.1} µs | | |", ns / 1e3),
        }
        eprintln!("NATIVE {} {ns}", name.split(',').next().unwrap().replace(' ', "_"));
    };

    // Poseidon2: one hash of two elements (one permutation), PVM 428,839 a hash (#41)
    let x = ark_bn254::Fr::from(7u64);
    row("poseidon2 hash (one permutation), p2", median_ns(|| { black_box(p2::hash(&[black_box(x), x])); }), Some(428_839.0));

    // KZG point evaluation: geth's precompile vector, PVM 199,473,075 (#24, kind 0)
    let g = fs::read_to_string(format!("{}/geth-pointEvaluation.json", a[1])).unwrap();
    let input = unhex(g.split("\"Input\": \"").nth(1).unwrap().split('"').next().unwrap());
    assert_eq!(kzg_verify::point_evaluation(&input), Ok(true));
    row("kzg_point_evaluation, kzg-verify (zkcrypto bls12_381)", median_ns(|| { black_box(kzg_verify::point_evaluation(black_box(&input))).ok(); }), Some(199_473_075.0));

    // ecrecover: the first generated signature, PVM 13,119,449 for one (#39)
    let line = fs::read_to_string(&a[2]).unwrap().lines().next().unwrap().to_string();
    let ec_in = unhex(line.split_whitespace().next().unwrap());
    assert!(ecrecover::ecrecover(&ec_in).is_some());
    row("secp256k1_recover, k256", median_ns(|| { black_box(ecrecover::ecrecover(black_box(&ec_in))); }), Some(13_119_449.0));

    // BN254 (arkworks): pairing checks and G1 MSM
    for k in [2usize, 4] {
        let ps: Vec<G1Affine> = (0..k).map(|_| G1Projective::rand(&mut rng).into_affine()).collect();
        let qs: Vec<G2Affine> = (0..k).map(|_| ark_bn254::G2Projective::rand(&mut rng).into_affine()).collect();
        row(&format!("bn254_pairing_check, {k} pairs, arkworks"), median_ns(|| { black_box(Bn254::multi_pairing(black_box(&ps), &qs)); }), None);
    }
    for n in [1usize, 16, 128] {
        let ps: Vec<G1Affine> = (0..n).map(|_| G1Projective::rand(&mut rng).into_affine()).collect();
        let ss: Vec<BnFr> = (0..n).map(|_| BnFr::rand(&mut rng)).collect();
        row(&format!("bn254_g1_msm, {n} points, arkworks"), median_ns(|| { black_box(G1Projective::msm(black_box(&ps), &ss).unwrap()); }), None);
    }
    let _ = G1Affine::generator();

    // BLS12-381 (blst): pairing check of 2 pairs, G1 MSM
    unsafe {
        use blst::*;
        let g1 = *blst_p1_affine_generator();
        let g2 = *blst_p2_affine_generator();
        let pair = || {
            let mut ml = blst_fp12::default();
            blst_miller_loop(&mut ml, &g2, &g1);
            let mut ml2 = blst_fp12::default();
            blst_miller_loop(&mut ml2, &g2, &g1);
            let mut prod = blst_fp12::default();
            blst_fp12_mul(&mut prod, &ml, &ml2);
            let mut out = blst_fp12::default();
            blst_final_exp(&mut out, &prod);
            black_box(out);
        };
        row("bls12_381_pairing_check, 2 pairs, blst", median_ns(pair), None);
        for n in [1usize, 16, 128] {
            let pts: Vec<blst_p1_affine> = (0..n).map(|_| g1).collect();
            let scalars: Vec<u8> = (0..n * 32).map(|i| (i * 7 + 3) as u8).collect();
            let p = blst::p1_affines::from(&pts.iter().map(|a| { let mut j = blst_p1::default(); blst_p1_from_affine(&mut j, a); j }).collect::<Vec<_>>());
            row(&format!("bls12_381_g1_msm, {n} points, blst"), median_ns(|| { black_box(p.mult(black_box(&scalars), 255)); }), None);
        }
    }
}
