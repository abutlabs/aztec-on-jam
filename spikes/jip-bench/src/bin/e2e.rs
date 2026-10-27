//! e2e (#45; call 105, lasair#130): the curve calls end to end, as a client would run them. Each takes its encoded
//! input (EIP-196/197 for BN254, EIP-2537 for BLS12-381), decodes and checks every element
//! (canonical field elements, on the curve, in the subgroup where the EIP requires it), and
//! computes the result. Scalars are any 32 bytes, as in the EIPs. Two implementations of each
//! curve run every input and must agree. Inputs are random points of the group, plus adversarial
//! cases. Printed: median, p99 and max of 101 single-threaded runs; stderr has
//! "E2E <call> <impl> <case> <size> <median ns> <p99 ns> <max ns>" for pricing.
use std::{hint::black_box, time::Instant};

fn stats<T>(mut f: impl FnMut() -> T) -> [f64; 3] {
    for _ in 0..3 { black_box(f()); }
    let mut t: Vec<f64> = (0..101).map(|_| { let s = Instant::now(); black_box(f()); s.elapsed().as_nanos() as f64 }).collect();
    t.sort_by(|a, b| a.partial_cmp(b).unwrap());
    [t[50], t[99], t[100]]
}

mod bn {
    //! BN254 on arkworks. G1: x ‖ y, 32 bytes each, big-endian; (0, 0) is infinity; cofactor 1,
    //! so on the curve is enough. G2: x_im ‖ x_re ‖ y_im ‖ y_re (EIP-197); on the curve and in
    //! the subgroup.
    use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine};
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
    use ark_ff::{BigInteger, One, PrimeField, Zero};
    pub fn fq(b: &[u8]) -> Option<Fq> {
        let v = Fq::from_be_bytes_mod_order(b);
        (v.into_bigint().to_bytes_be() == b).then_some(v)
    }
    pub fn g1(b: &[u8]) -> Option<G1Affine> {
        let (x, y) = (fq(&b[..32])?, fq(&b[32..64])?);
        if x.is_zero() && y.is_zero() { return Some(G1Affine::identity()); }
        let p = G1Affine::new_unchecked(x, y);
        p.is_on_curve().then_some(p)
    }
    pub fn g2(b: &[u8]) -> Option<G2Affine> {
        let x = Fq2::new(fq(&b[32..64])?, fq(&b[..32])?);
        let y = Fq2::new(fq(&b[96..128])?, fq(&b[64..96])?);
        if x.is_zero() && y.is_zero() { return Some(G2Affine::identity()); }
        let p = G2Affine::new_unchecked(x, y);
        (p.is_on_curve() && p.is_in_correct_subgroup_assuming_on_curve()).then_some(p)
    }
    pub fn enc1(p: &G1Affine) -> [u8; 64] {
        let mut o = [0u8; 64];
        if let Some((x, y)) = p.xy() {
            o[..32].copy_from_slice(&x.into_bigint().to_bytes_be());
            o[32..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        o
    }
    pub fn enc2(p: &G2Affine) -> [u8; 128] {
        let mut o = [0u8; 128];
        if let Some((x, y)) = p.xy() {
            for (i, f) in [x.c1, x.c0, y.c1, y.c0].iter().enumerate() {
                o[32 * i..32 * i + 32].copy_from_slice(&f.into_bigint().to_bytes_be());
            }
        }
        o
    }
    /// 101: n items of (G1 64, scalar 32).
    pub fn msm(input: &[u8]) -> Option<[u8; 64]> {
        if input.is_empty() || input.len() % 96 != 0 { return None; }
        let mut bases = Vec::new();
        let mut scalars = Vec::new();
        for item in input.chunks_exact(96) {
            bases.push(g1(&item[..64])?);
            scalars.push(Fr::from_be_bytes_mod_order(&item[64..]));
        }
        Some(enc1(&G1Projective::msm(&bases, &scalars).ok()?.into_affine()))
    }
    /// 102: k items of (G1 64, G2 128); k = 0 holds.
    pub fn pairing(input: &[u8]) -> Option<bool> {
        if input.len() % 192 != 0 { return None; }
        let mut a = Vec::new();
        let mut b = Vec::new();
        for item in input.chunks_exact(192) {
            a.push(g1(&item[..64])?);
            b.push(g2(&item[64..])?);
        }
        Some(Bn254::multi_pairing(a, b).0.is_one())
    }
}

mod bn_sub {
    //! BN254 on substrate-bn, the same encodings.
    use substrate_bn::{pairing_batch, AffineG1, AffineG2, Fq, Fq2, Fr, Group, Gt, G1, G2};
    fn fq(b: &[u8]) -> Option<Fq> { Fq::from_slice(b).ok() }
    fn g1(b: &[u8]) -> Option<G1> {
        let (x, y) = (fq(&b[..32])?, fq(&b[32..64])?);
        if x.is_zero() && y.is_zero() { return Some(G1::zero()); }
        AffineG1::new(x, y).ok().map(Into::into)
    }
    fn g2(b: &[u8]) -> Option<G2> {
        let x = Fq2::new(fq(&b[32..64])?, fq(&b[..32])?);
        let y = Fq2::new(fq(&b[96..128])?, fq(&b[64..96])?);
        if x.is_zero() && y.is_zero() { return Some(G2::zero()); }
        AffineG2::new(x, y).ok().map(Into::into)
    }
    fn enc1(p: G1) -> [u8; 64] {
        let mut o = [0u8; 64];
        if let Some(a) = AffineG1::from_jacobian(p) {
            a.x().to_big_endian(&mut o[..32]).unwrap();
            a.y().to_big_endian(&mut o[32..]).unwrap();
        }
        o
    }
    /// 101 as a sum of scalar multiplications (substrate-bn has no MSM).
    pub fn msm(input: &[u8]) -> Option<[u8; 64]> {
        if input.is_empty() || input.len() % 96 != 0 { return None; }
        let mut acc = G1::zero();
        for item in input.chunks_exact(96) {
            let s = Fr::from_slice(&super::reduce_bn(&item[64..])).ok()?;
            acc = acc + g1(&item[..64])? * s;
        }
        Some(enc1(acc))
    }
    pub fn pairing(input: &[u8]) -> Option<bool> {
        if input.len() % 192 != 0 { return None; }
        let mut pairs = Vec::new();
        for item in input.chunks_exact(192) {
            pairs.push((g1(&item[..64])?, g2(&item[64..])?));
        }
        Some(pairs.is_empty() || pairing_batch(&pairs) == Gt::one())
    }
}

fn reduce_bn(s: &[u8]) -> Vec<u8> {
    use ark_ff::{BigInteger, PrimeField};
    ark_bn254::Fr::from_be_bytes_mod_order(s).into_bigint().to_bytes_be()
}

mod bls {
    //! BLS12-381 on blst, EIP-2537's encodings: an Fp is 64 bytes, the top 16 zero; G1 x ‖ y
    //! (128), G2 x_c0 ‖ x_c1 ‖ y_c0 ‖ y_c1 (256); all zero is infinity; every point on the curve
    //! and in the subgroup.
    use blst::*;
    fn fp(b: &[u8]) -> Option<blst_fp> {
        if b[..16].iter().any(|&x| x != 0) { return None; }
        let mut f = blst_fp::default();
        let mut back = [0u8; 48];
        unsafe { blst_fp_from_bendian(&mut f, b[16..].as_ptr()); blst_bendian_from_fp(back.as_mut_ptr(), &f); }
        (back[..] == b[16..]).then_some(f)
    }
    pub fn g1(b: &[u8]) -> Option<blst_p1_affine> {
        let p = blst_p1_affine { x: fp(&b[..64])?, y: fp(&b[64..128])? };
        if b[..128].iter().all(|&x| x == 0) { return Some(blst_p1_affine::default()); }
        unsafe { (blst_p1_affine_on_curve(&p) && blst_p1_affine_in_g1(&p)).then_some(p) }
    }
    pub fn g2(b: &[u8]) -> Option<blst_p2_affine> {
        let p = blst_p2_affine {
            x: blst_fp2 { fp: [fp(&b[..64])?, fp(&b[64..128])?] },
            y: blst_fp2 { fp: [fp(&b[128..192])?, fp(&b[192..256])?] },
        };
        if b[..256].iter().all(|&x| x == 0) { return Some(blst_p2_affine::default()); }
        unsafe { (blst_p2_affine_on_curve(&p) && blst_p2_affine_in_g2(&p)).then_some(p) }
    }
    pub fn enc1(p: &blst_p1) -> [u8; 128] {
        let mut a = blst_p1_affine::default();
        let mut o = [0u8; 128];
        unsafe {
            blst_p1_to_affine(&mut a, p);
            if !blst_p1_affine_is_inf(&a) {
                blst_bendian_from_fp(o[16..64].as_mut_ptr(), &a.x);
                blst_bendian_from_fp(o[80..128].as_mut_ptr(), &a.y);
            }
        }
        o
    }
    /// 103: n items of (G1 128, scalar 32 big-endian).
    pub fn msm(input: &[u8]) -> Option<[u8; 128]> {
        if input.is_empty() || input.len() % 160 != 0 { return None; }
        let mut bases = Vec::new();
        let mut scalars = Vec::new();
        for item in input.chunks_exact(160) {
            bases.push(g1(&item[..128])?);
            scalars.extend(item[128..].iter().rev());
        }
        Some(enc1(&bases.as_slice().mult(&scalars, 256)))
    }
    /// 104: k items of (G1 128, G2 256); k = 0 holds.
    pub fn pairing(input: &[u8]) -> Option<bool> {
        if input.len() % 384 != 0 { return None; }
        let mut acc = unsafe { *blst_fp12_one() };
        let mut any = false;
        for item in input.chunks_exact(384) {
            let (p, q) = (g1(&item[..128])?, g2(&item[128..])?);
            unsafe {
                if blst_p1_affine_is_inf(&p) || blst_p2_affine_is_inf(&q) { continue; }
                let mut f = blst_fp12::default();
                blst_miller_loop(&mut f, &q, &p);
                let prev = acc;
                blst_fp12_mul(&mut acc, &prev, &f);
            }
            any = true;
        }
        if !any { return Some(true); }
        let mut out = blst_fp12::default();
        unsafe { blst_final_exp(&mut out, &acc); Some(blst_fp12_is_one(&out)) }
    }
}

mod bls_ark {
    //! BLS12-381 on arkworks, the same encodings.
    use ark_bls12_381::{Bls12_381, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine};
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
    use ark_ff::{BigInteger, One, PrimeField};
    fn fq(b: &[u8]) -> Option<Fq> {
        if b[..16].iter().any(|&x| x != 0) { return None; }
        let v = Fq::from_be_bytes_mod_order(&b[16..]);
        (v.into_bigint().to_bytes_be() == b[16..]).then_some(v)
    }
    fn g1(b: &[u8]) -> Option<G1Affine> {
        let (x, y) = (fq(&b[..64])?, fq(&b[64..128])?);
        if b[..128].iter().all(|&x| x == 0) { return Some(G1Affine::identity()); }
        let p = G1Affine::new_unchecked(x, y);
        (p.is_on_curve() && p.is_in_correct_subgroup_assuming_on_curve()).then_some(p)
    }
    fn g2(b: &[u8]) -> Option<G2Affine> {
        let x = Fq2::new(fq(&b[..64])?, fq(&b[64..128])?);
        let y = Fq2::new(fq(&b[128..192])?, fq(&b[192..256])?);
        if b[..256].iter().all(|&x| x == 0) { return Some(G2Affine::identity()); }
        let p = G2Affine::new_unchecked(x, y);
        (p.is_on_curve() && p.is_in_correct_subgroup_assuming_on_curve()).then_some(p)
    }
    pub fn msm(input: &[u8]) -> Option<[u8; 128]> {
        if input.is_empty() || input.len() % 160 != 0 { return None; }
        let mut bases = Vec::new();
        let mut scalars = Vec::new();
        for item in input.chunks_exact(160) {
            bases.push(g1(&item[..128])?);
            scalars.push(Fr::from_be_bytes_mod_order(&item[128..]));
        }
        let r = G1Projective::msm(&bases, &scalars).ok()?.into_affine();
        let mut o = [0u8; 128];
        if let Some((x, y)) = r.xy() {
            o[16..64].copy_from_slice(&x.into_bigint().to_bytes_be());
            o[80..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        Some(o)
    }
    pub fn pairing(input: &[u8]) -> Option<bool> {
        if input.len() % 384 != 0 { return None; }
        let mut a = Vec::new();
        let mut b = Vec::new();
        for item in input.chunks_exact(384) {
            a.push(g1(&item[..128])?);
            b.push(g2(&item[128..])?);
        }
        Some(Bls12_381::multi_pairing(a, b).0.is_one())
    }
}

mod gk {
    //! Grumpkin (call 105, lasair#130) on arkworks: y^2 = x^3 - 17 over BN254's scalar field r,
    //! prime order p (BN254's base modulus), cofactor 1. A point is x ‖ y, 32 bytes each,
    //! big-endian, below r; (0, 0) is infinity (never on the curve: -17 is not a square mod r).
    //! Scalars are any 32 bytes, reduced mod p.
    use ark_ec::{AffineRepr, CurveGroup, VariableBaseMSM};
    use ark_ff::{BigInteger, PrimeField, Zero};
    use ark_grumpkin::{Affine, Fq, Fr, Projective};
    pub fn fq(b: &[u8]) -> Option<Fq> {
        let v = Fq::from_be_bytes_mod_order(b);
        (v.into_bigint().to_bytes_be() == b).then_some(v)
    }
    pub fn point(b: &[u8]) -> Option<Affine> {
        let (x, y) = (fq(&b[..32])?, fq(&b[32..64])?);
        if x.is_zero() && y.is_zero() { return Some(Affine::identity()); }
        let p = Affine::new_unchecked(x, y);
        p.is_on_curve().then_some(p)
    }
    pub fn enc(p: &Affine) -> [u8; 64] {
        let mut o = [0u8; 64];
        if let Some((x, y)) = p.xy() {
            o[..32].copy_from_slice(&x.into_bigint().to_bytes_be());
            o[32..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        o
    }
    /// 105: n items of (point 64, scalar 32).
    pub fn msm(input: &[u8]) -> Option<[u8; 64]> {
        if input.is_empty() || input.len() % 96 != 0 { return None; }
        let mut bases = Vec::new();
        let mut scalars = Vec::new();
        for item in input.chunks_exact(96) {
            bases.push(point(&item[..64])?);
            scalars.push(Fr::from_be_bytes_mod_order(&item[64..]));
        }
        Some(enc(&Projective::msm(&bases, &scalars).ok()?.into_affine()))
    }
}

mod gk_h2c {
    //! Grumpkin on halo2curves, written apart from arkworks: its own field and curve code, its
    //! own canonical-encoding check (from_repr) and on-curve check (from_xy), and its serial
    //! Pippenger. The same encoding as `gk`.
    use halo2curves::ff::{FromUniformBytes, PrimeField};
    use halo2curves::group::{prime::PrimeCurveAffine, Curve, Group};
    use halo2curves::grumpkin::{Fq, Fr, G1Affine, G1};
    use halo2curves::CurveAffine;
    fn fq(b: &[u8]) -> Option<Fq> {
        let mut le = [0u8; 32];
        le.copy_from_slice(b);
        le.reverse();
        Option::from(Fq::from_repr(le.into()))
    }
    fn point(b: &[u8]) -> Option<G1Affine> {
        if b[..64].iter().all(|&x| x == 0) { return Some(G1Affine::identity()); }
        Option::from(G1Affine::from_xy(fq(&b[..32])?, fq(&b[32..64])?))
    }
    fn scalar(b: &[u8]) -> Fr {
        let mut wide = [0u8; 64];
        for i in 0..32 { wide[i] = b[31 - i]; }
        Fr::from_uniform_bytes(&wide)
    }
    pub fn msm(input: &[u8]) -> Option<[u8; 64]> {
        if input.is_empty() || input.len() % 96 != 0 { return None; }
        let mut bases = Vec::new();
        let mut scalars = Vec::new();
        for item in input.chunks_exact(96) {
            bases.push(point(&item[..64])?);
            scalars.push(scalar(&item[64..]));
        }
        let mut acc = G1::identity();
        halo2curves::msm::msm_serial(&scalars, &bases, &mut acc);
        let a = acc.to_affine();
        let mut o = [0u8; 64];
        if !bool::from(a.is_identity()) {
            let (mut x, mut y) = (a.x.to_repr(), a.y.to_repr());
            x.as_mut().reverse();
            y.as_mut().reverse();
            o[..32].copy_from_slice(x.as_ref());
            o[32..].copy_from_slice(y.as_ref());
        }
        Some(o)
    }
}

// --- inputs -------------------------------------------------------------------------------

use ark_ec::{AffineRepr, CurveGroup, PrimeGroup};
use ark_ff::{BigInteger, PrimeField, UniformRand};
use ark_std::rand::{rngs::StdRng, SeedableRng};

fn be32<F: PrimeField>(f: F) -> Vec<u8> { let mut v = f.into_bigint().to_bytes_be(); while v.len() < 32 { v.insert(0, 0); } v }
fn pad64(b: Vec<u8>) -> Vec<u8> { let mut o = vec![0u8; 64 - b.len()]; o.extend(b); o }

fn bls_g1(p: &ark_bls12_381::G1Affine) -> Vec<u8> {
    match p.xy() { Some((x, y)) => [pad64(x.into_bigint().to_bytes_be()), pad64(y.into_bigint().to_bytes_be())].concat(), None => vec![0; 128] }
}
fn bls_g2(p: &ark_bls12_381::G2Affine) -> Vec<u8> {
    match p.xy() {
        Some((x, y)) => [x.c0, x.c1, y.c0, y.c1].iter().flat_map(|f| pad64(f.into_bigint().to_bytes_be())).collect(),
        None => vec![0; 256],
    }
}

/// (case, n items) → input bytes, for one call.
fn cases(call: &str, n: usize, rng: &mut StdRng) -> Vec<(&'static str, Vec<u8>)> {
    use ark_bn254::{Fr as BnFr, G1Projective as Bn1, G2Projective as Bn2};
    use ark_bls12_381::{Fr as BlsFr, G1Projective as Bls1, G2Projective as Bls2};
    let r1 = -BnFr::from(1u64); // r − 1
    let rb = -BlsFr::from(1u64);
    let mut out = Vec::new();
    match call {
        "bn254_g1_msm" => {
            let pts: Vec<_> = (0..n).map(|_| (Bn1::generator() * BnFr::rand(rng)).into_affine()).collect();
            let sc: Vec<_> = (0..n).map(|_| BnFr::rand(rng)).collect();
            out.push(("random", (0..n).flat_map(|i| [bn::enc1(&pts[i]).to_vec(), be32(sc[i])].concat()).collect()));
            out.push(("one base repeated", (0..n).flat_map(|i| [bn::enc1(&pts[0]).to_vec(), be32(sc[i])].concat()).collect()));
            out.push(("scalars r − 1", (0..n).flat_map(|i| [bn::enc1(&pts[i]).to_vec(), be32(r1)].concat()).collect()));
            out.push(("P and −P", (0..n).flat_map(|i| { let p = if i % 2 == 0 { pts[i / 2 * 2] } else { -pts[i / 2 * 2] }; [bn::enc1(&p).to_vec(), be32(sc[0])].concat() }).collect()));
            out.push(("half infinity", (0..n).flat_map(|i| [if i % 2 == 0 { [0u8; 64].to_vec() } else { bn::enc1(&pts[i]).to_vec() }, be32(sc[i])].concat()).collect()));
            out.push(("scalars 2^256 − 1", (0..n).flat_map(|i| [bn::enc1(&pts[i]).to_vec(), vec![0xff; 32]].concat()).collect()));
        }
        "grumpkin_g1_msm" => {
            use ark_grumpkin::{Fr as GkFr, Projective as Gk};
            let p1 = -GkFr::from(1u64); // p − 1
            let pts: Vec<_> = (0..n).map(|_| (Gk::generator() * GkFr::rand(rng)).into_affine()).collect();
            let sc: Vec<_> = (0..n).map(|_| GkFr::rand(rng)).collect();
            out.push(("random", (0..n).flat_map(|i| [gk::enc(&pts[i]).to_vec(), be32(sc[i])].concat()).collect()));
            out.push(("one base repeated", (0..n).flat_map(|i| [gk::enc(&pts[0]).to_vec(), be32(sc[i])].concat()).collect()));
            out.push(("scalars p − 1", (0..n).flat_map(|i| [gk::enc(&pts[i]).to_vec(), be32(p1)].concat()).collect()));
            out.push(("P and −P", (0..n).flat_map(|i| { let p = if i % 2 == 0 { pts[i / 2 * 2] } else { -pts[i / 2 * 2] }; [gk::enc(&p).to_vec(), be32(sc[0])].concat() }).collect()));
            out.push(("half infinity", (0..n).flat_map(|i| [if i % 2 == 0 { [0u8; 64].to_vec() } else { gk::enc(&pts[i]).to_vec() }, be32(sc[i])].concat()).collect()));
            out.push(("scalars 2^256 − 1", (0..n).flat_map(|i| [gk::enc(&pts[i]).to_vec(), vec![0xff; 32]].concat()).collect()));
        }
        "bn254_pairing_check" => {
            // k pairs whose product is 1: e(a_i G1, b_i G2) for random a, b, with the last pair cancelling
            let (mut items, mut acc) = (Vec::new(), BnFr::from(0u64));
            for i in 0..n {
                let (a, b) = (BnFr::rand(rng), BnFr::rand(rng));
                let (a, b) = if i + 1 == n && n > 1 { (-acc, BnFr::from(1u64)) } else { acc += a * b; (a, b) };
                items.push([bn::enc1(&(Bn1::generator() * a).into_affine()).to_vec(), bn::enc2(&(Bn2::generator() * b).into_affine()).to_vec()].concat());
            }
            out.push((if n > 1 { "random, holds" } else { "random, fails" }, items.concat()));
            let one = items[0].clone();
            out.push(("one pair repeated", (0..n).flat_map(|_| one.clone()).collect()));
            // the last G2 on the twist but outside the subgroup: must be refused
            let mut bad = items.clone();
            let q = loop {
                let x = ark_bn254::Fq2::rand(rng);
                if let Some(q) = ark_bn254::G2Affine::get_point_from_x_unchecked(x, true) {
                    if !q.is_in_correct_subgroup_assuming_on_curve() { break q; }
                }
            };
            bad.last_mut().unwrap()[64..].copy_from_slice(&bn::enc2(&q));
            out.push(("last G2 outside the subgroup", bad.concat()));
        }
        "bls12_381_g1_msm" => {
            let pts: Vec<_> = (0..n).map(|_| (Bls1::generator() * BlsFr::rand(rng)).into_affine()).collect();
            let sc: Vec<_> = (0..n).map(|_| BlsFr::rand(rng)).collect();
            out.push(("random", (0..n).flat_map(|i| [bls_g1(&pts[i]), be32(sc[i])].concat()).collect()));
            out.push(("one base repeated", (0..n).flat_map(|i| [bls_g1(&pts[0]), be32(sc[i])].concat()).collect()));
            out.push(("scalars r − 1", (0..n).flat_map(|i| [bls_g1(&pts[i]), be32(rb)].concat()).collect()));
            out.push(("P and −P", (0..n).flat_map(|i| { let p = if i % 2 == 0 { pts[i / 2 * 2] } else { -pts[i / 2 * 2] }; [bls_g1(&p), be32(sc[0])].concat() }).collect()));
            out.push(("half infinity", (0..n).flat_map(|i| [if i % 2 == 0 { vec![0u8; 128] } else { bls_g1(&pts[i]) }, be32(sc[i])].concat()).collect()));
            out.push(("scalars 2^256 − 1", (0..n).flat_map(|i| [bls_g1(&pts[i]), vec![0xff; 32]].concat()).collect()));
        }
        "bls12_381_pairing_check" => {
            let (mut items, mut acc) = (Vec::new(), BlsFr::from(0u64));
            for i in 0..n {
                let (a, b) = (BlsFr::rand(rng), BlsFr::rand(rng));
                let (a, b) = if i + 1 == n && n > 1 { (-acc, BlsFr::from(1u64)) } else { acc += a * b; (a, b) };
                items.push([bls_g1(&(Bls1::generator() * a).into_affine()), bls_g2(&(Bls2::generator() * b).into_affine())].concat());
            }
            out.push((if n > 1 { "random, holds" } else { "random, fails" }, items.concat()));
            let one = items[0].clone();
            out.push(("one pair repeated", (0..n).flat_map(|_| one.clone()).collect()));
            let mut bad = items.clone();
            let q = loop {
                let x = ark_bls12_381::Fq2::rand(rng);
                if let Some(q) = ark_bls12_381::G2Affine::get_point_from_x_unchecked(x, true) {
                    if !q.is_in_correct_subgroup_assuming_on_curve() { break q; }
                }
            };
            let enc = bls_g2(&q);
            bad.last_mut().unwrap()[128..].copy_from_slice(&enc);
            out.push(("last G2 outside the subgroup", bad.concat()));
        }
        _ => unreachable!(),
    }
    out
}

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

/// `e2e --vectors`: JSON lines of {call, case, input, result, output} for small inputs, from both
/// implementations (which must agree), plus malformed inputs that must return HUH.
fn vectors() {
    let mut rng = StdRng::seed_from_u64(4545);
    let run = |call: &str, i: &[u8]| -> [Option<Vec<u8>>; 2] {
        match call {
            "bn254_g1_msm" => [bn::msm(i).map(|o| o.to_vec()), bn_sub::msm(i).map(|o| o.to_vec())],
            "bn254_pairing_check" => [bn::pairing(i).map(|o| vec![o as u8]), bn_sub::pairing(i).map(|o| vec![o as u8])],
            "bls12_381_g1_msm" => [bls::msm(i).map(|o| o.to_vec()), bls_ark::msm(i).map(|o| o.to_vec())],
            "grumpkin_g1_msm" => [gk::msm(i).map(|o| o.to_vec()), gk_h2c::msm(i).map(|o| o.to_vec())],
            _ => [bls::pairing(i).map(|o| vec![o as u8]), bls_ark::pairing(i).map(|o| vec![o as u8])],
        }
    };
    let emit = |call: &str, case: &str, input: &[u8]| {
        let [a, b] = run(call, input);
        assert_eq!(a, b, "{call} {case}: the implementations differ");
        let (result, output) = match a {
            None => ("HUH", String::new()),
            Some(o) if call.contains("pairing") => ("OK", format!("{}", o[0])),
            Some(o) => ("OK", hex(&o)),
        };
        let field = if call.contains("pairing") { "phi8" } else { "output" };
        println!("{{\"call\": \"{call}\", \"case\": \"{case}\", \"input\": \"{}\", \"result\": \"{result}\", \"{field}\": \"{output}\"}}", hex(input));
    };
    for call in ["bn254_g1_msm", "bn254_pairing_check", "bls12_381_g1_msm", "bls12_381_pairing_check", "grumpkin_g1_msm"] {
        for n in [1usize, 2, 4] {
            for (case, input) in cases(call, n, &mut rng) {
                emit(call, &format!("{case}, n = {n}"), &input);
            }
        }
        let (item, fe) = match call { "bn254_g1_msm" | "grumpkin_g1_msm" => (96, 32), "bn254_pairing_check" => (192, 32), "bls12_381_g1_msm" => (160, 64), _ => (384, 64) };
        let good = cases(call, 1, &mut rng).remove(0).1;
        emit(call, "length not a multiple of the item", &good[..item - 1]);
        if call.contains("pairing") { emit(call, "no pairs (holds)", &[]); } else { emit(call, "no items", &[]); }
        // x = p: not a canonical field element
        let mut bad = good.clone();
        if call == "grumpkin_g1_msm" {
            // Grumpkin's coordinates are below r, BN254's scalar modulus; y as well as x
            let m = <ark_grumpkin::Fq as PrimeField>::MODULUS.to_bytes_be();
            bad[..32].copy_from_slice(&m);
            let mut bad_y = good.clone();
            bad_y[32..64].copy_from_slice(&m);
            emit(call, "y equal to the field modulus", &bad_y);
        } else if fe == 32 {
            let m = <ark_bn254::Fq as PrimeField>::MODULUS.to_bytes_be();
            bad[..32].copy_from_slice(&m);
        } else {
            let m = <ark_bls12_381::Fq as PrimeField>::MODULUS.to_bytes_be();
            bad[16..64].copy_from_slice(&m);
        }
        emit(call, "x equal to the field modulus", &bad);
        let mut off = good.clone();
        let last = if fe == 32 { 63 } else { 127 };
        off[last] ^= 1;
        emit(call, if call == "grumpkin_g1_msm" { "point off the curve" } else { "G1 point off the curve" }, &off);
        if fe == 64 {
            let mut top = good.clone();
            top[0] = 1;
            emit(call, "a field element's top 16 bytes not zero", &top);
            let q = loop {
                let x = ark_bls12_381::Fq::rand(&mut rng);
                if let Some(q) = ark_bls12_381::G1Affine::get_point_from_x_unchecked(x, true) {
                    if !q.is_in_correct_subgroup_assuming_on_curve() { break q; }
                }
            };
            let mut sub = good.clone();
            sub[..128].copy_from_slice(&bls_g1(&q));
            emit(call, "G1 point outside the subgroup", &sub);
        }
    }
    // The review of lasair#130: x = modulus reduces to (0, y), which is off every one of these
    // curves, so it cannot tell a client that rejects a non-canonical coordinate from one that
    // reduces it. A valid point with a modulus added to one coordinate can: canonical, it is
    // HUH; reduced, it is the point. One for each coordinate, for each MSM.
    for call in ["bn254_g1_msm", "bls12_381_g1_msm", "grumpkin_g1_msm"] {
        let good = cases(call, 1, &mut rng).remove(0).1;
        let (fe, m) = match call {
            "bn254_g1_msm" => (32, <ark_bn254::Fq as PrimeField>::MODULUS.to_bytes_be()),
            "grumpkin_g1_msm" => (32, <ark_grumpkin::Fq as PrimeField>::MODULUS.to_bytes_be()),
            _ => (64, <ark_bls12_381::Fq as PrimeField>::MODULUS.to_bytes_be()),
        };
        for (c, name) in [(0, "x plus the field modulus"), (1, "y plus the field modulus")] {
            let mut v = good.clone();
            // the coordinate's value octets: all 32, or the low 48 of an EIP-2537 field
            let lo = fe * c + if fe == 64 { 16 } else { 0 };
            let w = m.len();
            let mut carry = 0u16;
            for i in (0..w).rev() {
                let t = v[lo + i] as u16 + m[m.len() - w + i] as u16 + carry;
                v[lo + i] = t as u8;
                carry = t >> 8;
            }
            assert_eq!(carry, 0, "{call} {name}: the sum overflows its field");
            emit(call, name, &v);
        }
    }
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--vectors") { return vectors(); }
    // `e2e CALL` times only that call (call 105 was added after 101-104 were priced)
    let only = std::env::args().nth(1);
    let mut rng = StdRng::seed_from_u64(45);
    let (mut agree, mut differ) = (0, 0);
    println!("| call | implementation | case | n | result | median | p99 | max |");
    println!("|---|---|---|---|---|---|---|---|");
    let us = |x: f64| format!("{:.1} µs", x / 1e3);
    for (call, sizes) in [
        ("bn254_g1_msm", vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512]),
        ("bn254_pairing_check", vec![1, 2, 4, 8]),
        ("bls12_381_g1_msm", vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512]),
        ("bls12_381_pairing_check", vec![1, 2, 4, 8]),
        // 101's sizes, then to 2^15, the size of Aztec's IPA (lasair#130)
        ("grumpkin_g1_msm", vec![1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768]),
    ] {
        if only.as_deref().is_some_and(|c| c != call) { continue; }
        for n in sizes {
            for (case, input) in cases(call, n, &mut rng) {
                type Run = Box<dyn Fn(&[u8]) -> Option<Vec<u8>>>;
                let impls: [(&str, Run); 2] = match call {
                    "bn254_g1_msm" => [("arkworks", Box::new(|i| bn::msm(i).map(|o| o.to_vec()))), ("substrate-bn, sum of muls", Box::new(|i| bn_sub::msm(i).map(|o| o.to_vec())))],
                    "bn254_pairing_check" => [("arkworks", Box::new(|i| bn::pairing(i).map(|o| vec![o as u8]))), ("substrate-bn", Box::new(|i| bn_sub::pairing(i).map(|o| vec![o as u8])))],
                    "bls12_381_g1_msm" => [("blst", Box::new(|i| bls::msm(i).map(|o| o.to_vec()))), ("arkworks", Box::new(|i| bls_ark::msm(i).map(|o| o.to_vec())))],
                    "grumpkin_g1_msm" => [("arkworks", Box::new(|i| gk::msm(i).map(|o| o.to_vec()))), ("halo2curves", Box::new(|i| gk_h2c::msm(i).map(|o| o.to_vec())))],
                    _ => [("blst", Box::new(|i| bls::pairing(i).map(|o| vec![o as u8]))), ("arkworks", Box::new(|i| bls_ark::pairing(i).map(|o| vec![o as u8])))],
                };
                let outs: Vec<_> = impls.iter().map(|(_, f)| f(&input)).collect();
                if outs[0] == outs[1] { agree += 1 } else { differ += 1; eprintln!("DIFFER {call} {case} {n}: {:?} / {:?}", outs[0].as_ref().map(|o| o.len()), outs[1].as_ref().map(|o| o.len())); }
                let result = match &outs[0] { None => "HUH".to_string(), Some(o) if o.len() == 1 => format!("{}", o[0]), Some(_) => "OK".to_string() };
                for (name, f) in &impls {
                    let [m, p99, max] = stats(|| f(black_box(&input)));
                    println!("| {call} | {name} | {case} | {n} | {result} | {} | {} | {} |", us(m), us(p99), us(max));
                    eprintln!("E2E {call} {} {} {n} {m} {p99} {max}", name.split(',').next().unwrap().replace(' ', "_"), case.replace(' ', "_"));
                }
            }
        }
    }
    println!();
    println!("Implementations agree on {agree} inputs and differ on {differ}.");
}
