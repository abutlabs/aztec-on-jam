//! #58: HostCallHooks' EIP-196/197 encodings against arkworks, through the hooks themselves (with
//! the native stand-in for calls 101 and 102): points at infinity, the generator, random points,
//! scalars from 0 to r - 1, repeated bases, and pairings that hold and fail.

use ark_bn254::{Fr, G1Affine, G1Projective, G2Affine};
use ark_bn254_ext::CurveHooks;
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
use ark_ff::{One, UniformRand, Zero};
use ark_std::rand::{rngs::StdRng, SeedableRng};
use curve_hooks::{enc_g1, enc_g2, host_calls_stand_in, host_calls_used, HostCallHooks, FAILED};
use std::sync::Mutex;

// the detection state is global, as in a refine: one test at a time
static SERIAL: Mutex<()> = Mutex::new(());

type H = HostCallHooks;
type ExtG1 = ark_bn254_ext::g1::G1Affine<H>;
type ExtG2 = ark_bn254_ext::g2::G2Affine<H>;
type Ext = ark_bn254_ext::Bn254<H>;

fn ext1(p: &G1Affine) -> ExtG1 {
    if p.is_zero() { ExtG1::identity() } else { ExtG1::new_unchecked(p.x, p.y) }
}
fn ext2(p: &G2Affine) -> ExtG2 {
    if p.is_zero() { ExtG2::identity() } else { ExtG2::new_unchecked(p.x, p.y) }
}

#[test]
fn msm_and_pairing_through_the_calls_equal_arkworks() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    host_calls_stand_in::set_absent(false);
    let mut rng = StdRng::seed_from_u64(58);
    let g = G1Affine::generator();
    let cases: Vec<(&str, Vec<G1Affine>, Vec<Fr>)> = vec![
        ("the generator, scalar 1", vec![g], vec![Fr::one()]),
        ("infinity", vec![G1Affine::zero()], vec![Fr::from(5u8)]),
        ("scalar 0", vec![g], vec![Fr::zero()]),
        ("scalar r - 1", vec![g], vec![-Fr::one()]),
        ("a base repeated", vec![g, g, g], vec![Fr::from(2u8), Fr::from(3u8), -Fr::from(5u8)]),
        ("random, 62 points (Aztec's 2^24 MSM)", (0..62).map(|_| G1Affine::rand(&mut rng)).collect(), (0..62).map(|_| Fr::rand(&mut rng)).collect()),
        ("random with infinities", (0..9).map(|i| if i % 3 == 0 { G1Affine::zero() } else { G1Affine::rand(&mut rng) }).collect(), (0..9).map(|_| Fr::rand(&mut rng)).collect()),
    ];
    for (what, bases, scalars) in &cases {
        let want = G1Projective::msm(bases, scalars).unwrap().into_affine();
        let ext: Vec<ExtG1> = bases.iter().map(ext1).collect();
        let got = <H as CurveHooks>::bn254_msm_g1(&ext, scalars).unwrap().into_affine();
        assert_eq!((got.x, got.y, got.infinity), (want.x, want.y, want.infinity), "{what}");
        assert_eq!(host_calls_used().0, Some(true), "{what}: by call 101");
    }
    // pairings: a product that is one, one that is not, infinity in either place
    let (a, b) = (Fr::rand(&mut rng), Fr::rand(&mut rng));
    let (p, q) = ((g * a).into_affine(), (G2Affine::generator() * b).into_affine());
    let holds = [(p, q), ((-(g * (a * b))).into_affine(), G2Affine::generator())];
    let fails = [(p, q), (g, G2Affine::generator())];
    for (what, pairs, expect) in [("holds", &holds[..], true), ("fails", &fails[..], false),
                                  ("infinity in G1", &[(G1Affine::zero(), q)][..], true),
                                  ("infinity in G2", &[(p, G2Affine::zero())][..], true)] {
        let want = ark_bn254::Bn254::multi_pairing(pairs.iter().map(|x| x.0), pairs.iter().map(|x| x.1)).0.is_one();
        assert_eq!(want, expect, "{what}: arkworks");
        let got = Ext::multi_pairing(pairs.iter().map(|x| ext1(&x.0)), pairs.iter().map(|x| ext2(&x.1))).0.is_one();
        assert_eq!(got, expect, "{what}: through call 102");
        assert_eq!(host_calls_used().1, Some(true), "{what}: by call 102");
    }
}

#[test]
fn a_failed_check_never_becomes_one() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    // FAILED is e(G1, G2), and stays not one through a real final exponentiation (the review
    // of #58: a non-one value of Fp became one there)
    let e = ark_bn254::Bn254::pairing(G1Affine::generator(), G2Affine::generator()).0;
    assert_eq!(FAILED, e, "FAILED is e(G1, G2)");
    assert!(!FAILED.is_one());
    let fe = ark_bn254::Bn254::final_exponentiation(ark_ec::pairing::MillerLoopOutput(FAILED)).unwrap().0;
    assert!(!fe.is_one(), "a real final exponentiation keeps it not one");
    host_calls_stand_in::set_absent(false);
    let r = Ext::multi_pairing([ext1(&G1Affine::generator())], [ext2(&G2Affine::generator())]);
    assert!(!r.0.is_one(), "e(G, H) through call 102 does not hold");
    // the hook's verdict for a failed check, put through a real final exponentiation: still not one
    host_calls_stand_in::set_absent(false);
    let v = Ext::multi_miller_loop([ext1(&G1Affine::generator())], [ext2(&G2Affine::generator())]).0;
    let fe = ark_bn254::Bn254::final_exponentiation(ark_ec::pairing::MillerLoopOutput(v)).unwrap().0;
    assert!(!fe.is_one(), "a failed verdict meeting a real final exponentiation stays not one");
}

#[test]
fn encodings_are_eip_196_and_197_bytes() {
    let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let hex = |s: &str| -> Vec<u8> { (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect() };
    let mut b = Vec::new();
    enc_g1(&mut b, &ext1(&G1Affine::generator()));
    assert_eq!(b, [[0u8; 31].as_slice(), &[1], &[0; 31], &[2]].concat(), "G1's generator is (1, 2)");
    let mut b = Vec::new();
    enc_g1(&mut b, &ext1(&G1Affine::zero()));
    assert_eq!(b, vec![0; 64], "infinity is all zero");
    // EIP-197's G2 generator, imaginary parts first (go-ethereum's bn256Pairing vectors)
    let mut b = Vec::new();
    enc_g2(&mut b, &ext2(&G2Affine::generator()));
    assert_eq!(b, hex(concat!(
        "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2",
        "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed",
        "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b",
        "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa")), "G2's generator, EIP-197's order");
    // 2 * G by call 101: the scalar's bytes and the result's, against the published value
    host_calls_stand_in::set_absent(false);
    let r = <H as CurveHooks>::bn254_msm_g1(&[ext1(&G1Affine::generator())], &[Fr::from(2u8)]).unwrap().into_affine();
    let mut b = Vec::new();
    enc_g1(&mut b, &r);
    assert_eq!(b, hex(concat!(
        "030644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd3",
        "15ed738c0e0a7c92e7845f96b2ae9c0a68a6a449e3538fc7ff3ebf7a5a18a2c4")), "2G");
}
