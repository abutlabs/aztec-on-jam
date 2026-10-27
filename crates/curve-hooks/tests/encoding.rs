//! #58: HostCallHooks' EIP-196/197 encodings against arkworks, through the hooks themselves (with
//! the native stand-in for calls 101 and 102): points at infinity, the generator, random points,
//! scalars from 0 to r - 1, repeated bases, and pairings that hold and fail.

use ark_bn254::{Fr, G1Affine, G1Projective, G2Affine};
use ark_bn254_ext::CurveHooks;
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
use ark_ff::{One, UniformRand, Zero};
use ark_std::rand::{rngs::StdRng, SeedableRng};
use curve_hooks::{host_calls_stand_in, host_calls_used, HostCallHooks};

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
fn failed_checks_never_compare_equal() {
    host_calls_stand_in::set_absent(false);
    let g = G1Affine::generator();
    let h = G2Affine::generator();
    let one = Ext::multi_pairing([ext1(&g)], [ext2(&h)]);
    let two = Ext::multi_pairing([ext1(&g)], [ext2(&h)]);
    assert!(!one.0.is_one() && !two.0.is_one());
    assert_ne!(one, two, "two failed checks are distinct verdicts");
}
