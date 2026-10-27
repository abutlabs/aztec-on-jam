//! EIP-4844's KZG point evaluation, as Ethereum's 0x0a precompile checks it (Aztec's
//! `BlobLib.sol` calls it once per epoch proof): the commitment C opens to y at z, with proof
//! pi, against Ethereum's trusted setup. c-kzg-4844's `verify_kzg_proof`:
//!   e(pi, [s]G2 - [z]G2) == e(C - [y]G1, G2)
//! checked as one multi-Miller loop and one final exponentiation.
//! verify_blob_kzg_proof: a blob's 4,096 field elements open, at a Fiat-Shamir point, to what
//! its commitment says (the check Ethereum's consensus does for each blob, free to contracts).
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use ff::{Field, PrimeField};
use bls12_381::{multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective, Gt, Scalar};
use sha2::{Digest, Sha256};

/// [s]G2 from Ethereum's KZG ceremony: c-kzg-4844 `trusted_setup.txt`, the second G2 point.
pub const SETUP_G2_S: [u8; 96] = hex48x2(
    *b"b5bfd7dd8cdeb128843bc287230af38926187075cbfbefa81009a2ce615ac53d2914e5870cb452d2afaaab24f3499f72185cbfee53492714734429b7b38608e23926c911cceceac9a36851477ba4c60b087041de621000edc98edada20c1def2",
);

const fn hex48x2(h: [u8; 192]) -> [u8; 96] {
    const fn nib(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("hex"),
        }
    }
    let mut out = [0u8; 96];
    let mut i = 0;
    while i < 96 {
        out[i] = nib(h[2 * i]) << 4 | nib(h[2 * i + 1]);
        i += 1;
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// z or y is not a canonical BLS12-381 scalar (not below r)
    BadScalar,
    /// not a compressed G1 point in the prime-order subgroup
    BadPoint,
    /// the versioned hash is not 0x01 || sha256(C)[1..]
    BadVersionedHash,
    /// the input is not 192 bytes, or a blob is not 131,072
    BadLength,
}

/// EIP-4844: field elements per blob, and the Fiat-Shamir domain of c-kzg-4844's
/// compute_challenge.
pub const FIELD_ELEMENTS_PER_BLOB: usize = 4096;
pub const BYTES_PER_BLOB: usize = 32 * FIELD_ELEMENTS_PER_BLOB;
const FIAT_SHAMIR_DOMAIN: &[u8; 16] = b"FSBLOBVERIFY_V1_";

/// A big-endian 32-byte scalar, canonical only (as c-kzg's bytes_to_bls_field).
fn scalar(be: &[u8; 32]) -> Result<Scalar, Error> {
    let mut le = *be;
    le.reverse();
    Option::from(Scalar::from_bytes(&le)).ok_or(Error::BadScalar)
}

/// A compressed G1 point on the curve, its subgroup not yet checked ([in_subgroup], or call 103).
fn on_curve(b: &[u8; 48]) -> Result<G1Affine, Error> {
    Option::from(G1Affine::from_compressed_unchecked(b)).ok_or(Error::BadPoint)
}

fn in_subgroup(ps: &[G1Affine]) -> Result<(), Error> {
    if ps.iter().all(|p| bool::from(p.is_torsion_free())) { Ok(()) } else { Err(Error::BadPoint) }
}

/// c-kzg-4844's verify_kzg_proof: Ok(true) if C opens to y at z.
pub fn verify_kzg_proof(c: &[u8; 48], z: &[u8; 32], y: &[u8; 32], pi: &[u8; 48]) -> Result<bool, Error> {
    // C and pi decompressed once, their subgroup checked by call 103 where the net has the
    // curve host calls, else here
    let (c, pi) = (on_curve(c)?, on_curve(pi)?);
    let checked = !calls::may_have();
    if checked {
        in_subgroup(&[c, pi])?;
    }
    let (z, y) = (scalar(z)?, scalar(y)?);
    // e(pi, [s]G2) * e(-C + [y]G1 - [z]pi, G2) == 1
    if let Some(r) = calls::check(&[(pi, Scalar::ONE)], &[(c, -Scalar::ONE), (G1Affine::generator(), y), (pi, -z)]) {
        return r;
    }
    if !checked {
        in_subgroup(&[c, pi])?; // no calls after all (103's probe said WHAT): check here
    }
    let s = G2Affine::from_compressed(&SETUP_G2_S).unwrap();
    let x_minus_z = G2Affine::from(G2Projective::from(s) - G2Affine::generator() * z);
    let c_minus_y = G1Affine::from(G1Projective::from(c) - G1Affine::generator() * y);
    // e(pi, [s - z]G2) * e(C - [y]G1, -G2) == 1
    let g2_neg = -G2Affine::generator();
    let ml = multi_miller_loop(&[(&pi, &G2Prepared::from(x_minus_z)), (&c_minus_y, &G2Prepared::from(g2_neg))]);
    Ok(ml.final_exponentiation() == Gt::identity())
}

/// The 0x0a precompile's input: versioned_hash (32) || z (32) || y (32) || C (48) || pi (48).
/// Ok(true) is the precompile's success; Ok(false) or Err is its failure.
pub fn point_evaluation(input: &[u8]) -> Result<bool, Error> {
    if input.len() != 192 {
        return Err(Error::BadLength);
    }
    let c: &[u8; 48] = input[96..144].try_into().unwrap();
    let mut vh: [u8; 32] = Sha256::digest(c).into();
    vh[0] = 0x01;
    if vh != input[..32] {
        return Err(Error::BadVersionedHash);
    }
    verify_kzg_proof(c, input[32..64].try_into().unwrap(), input[64..96].try_into().unwrap(), input[144..].try_into().unwrap())
}

/// The 4,096th roots of unity in bit-reversed order (c-kzg-4844's roots_of_unity_brp):
/// w = 7^((r - 1) / 4096), which is bls12_381's 2^32-th root of unity (from 7) to the 2^20.
fn roots_brp() -> Vec<Scalar> {
    let w = Scalar::ROOT_OF_UNITY.pow_vartime(&[1u64 << 20, 0, 0, 0]);
    let mut roots = Vec::with_capacity(FIELD_ELEMENTS_PER_BLOB);
    let mut x = Scalar::ONE;
    for _ in 0..FIELD_ELEMENTS_PER_BLOB {
        roots.push(x);
        x *= w;
    }
    let bits = FIELD_ELEMENTS_PER_BLOB.trailing_zeros();
    (0..FIELD_ELEMENTS_PER_BLOB).map(|i| roots[(i as u32).reverse_bits() as usize >> (32 - bits)]).collect()
}

/// c-kzg-4844's compute_challenge: sha256(domain || degree as 16 bytes big-endian || blob ||
/// commitment), reduced mod r (hash_to_bls_field).
fn challenge(blob: &[u8], c: &[u8; 48]) -> Scalar {
    let mut degree = [0u8; 16];
    degree[8..].copy_from_slice(&(FIELD_ELEMENTS_PER_BLOB as u64).to_be_bytes());
    let h: [u8; 32] = Sha256::new().chain_update(FIAT_SHAMIR_DOMAIN).chain_update(degree)
        .chain_update(blob).chain_update(c).finalize().into();
    let mut wide = [0u8; 64];
    for i in 0..32 {
        wide[i] = h[31 - i];
    }
    Scalar::from_bytes_wide(&wide)
}

/// c-kzg-4844's evaluate_polynomial_in_evaluation_form, barycentric over the bit-reversed
/// roots: p(z) = (z^n - 1) / n * sum_i p_i * w_i / (z - w_i), one batched inversion.
fn evaluate(poly: &[Scalar], z: Scalar, roots: &[Scalar]) -> Scalar {
    if let Some(i) = roots.iter().position(|w| *w == z) {
        return poly[i];
    }
    // Montgomery's batch inversion of (z - w_i)
    let diffs: Vec<Scalar> = roots.iter().map(|w| z - w).collect();
    let mut prefix = Vec::with_capacity(diffs.len());
    let mut acc = Scalar::ONE;
    for d in &diffs {
        prefix.push(acc);
        acc *= d;
    }
    let mut inv = acc.invert().unwrap(); // no d is zero: z is not a root
    let mut sum = Scalar::ZERO;
    for i in (0..diffs.len()).rev() {
        let inv_i = inv * prefix[i];
        inv *= diffs[i];
        sum += poly[i] * roots[i] * inv_i;
    }
    let n = Scalar::from(FIELD_ELEMENTS_PER_BLOB as u64);
    let zn = z.pow_vartime(&[FIELD_ELEMENTS_PER_BLOB as u64, 0, 0, 0]);
    sum * (zn - Scalar::ONE) * n.invert().unwrap()
}

/// A blob's Fiat-Shamir point z and its polynomial's value y there (c-kzg-4844's
/// compute_challenge and evaluate_polynomial_in_evaluation_form), after checking C and the blob.
fn blob_opening(blob: &[u8], c: &[u8; 48], roots: &[Scalar]) -> Result<(Scalar, Scalar), Error> {
    if blob.len() != BYTES_PER_BLOB {
        return Err(Error::BadLength);
    }
    let poly: Vec<Scalar> = blob.chunks_exact(32).map(|b| scalar(b.try_into().unwrap())).collect::<Result<_, _>>()?;
    let z = challenge(blob, c); // C is checked once, by the caller's opening check

    Ok((z, evaluate(&poly, z, roots)))
}

fn be(x: Scalar) -> [u8; 32] {
    let mut b = x.to_bytes();
    b.reverse();
    b
}

/// c-kzg-4844's verify_blob_kzg_proof: the blob's polynomial opens at the Fiat-Shamir point
/// to what C commits to.
pub fn verify_blob_kzg_proof(blob: &[u8], c: &[u8; 48], pi: &[u8; 48]) -> Result<bool, Error> {
    let (z, y) = blob_opening(blob, c, &roots_brp())?;
    verify_kzg_proof(c, &be(z), &be(y), pi)
}

/// c-kzg-4844's verify_blob_kzg_proof_batch: every blob opens to its commitment, checked with one
/// pairing over a random linear combination, r = hash_to_bls_field(sha256("RCKZGBATCH___V1_" ||
/// degree || n || (C_i || z_i || y_i || pi_i)...)):
///   e(sum r^i pi_i, [s]G2) == e(sum r^i (C_i - [y_i]G1) + sum r^i z_i pi_i, G2)
/// Each blob's hashing and evaluation remain; the pairing is paid once.
pub fn verify_blob_kzg_proof_batch(blobs: &[&[u8]], cs: &[[u8; 48]], pis: &[[u8; 48]]) -> Result<bool, Error> {
    let n = blobs.len();
    if cs.len() != n || pis.len() != n {
        return Err(Error::BadLength);
    }
    match n {
        0 => return Ok(true),
        1 => return verify_blob_kzg_proof(blobs[0], &cs[0], &pis[0]),
        _ => {}
    }
    let roots = roots_brp();
    let mut zs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    let mut cps = Vec::with_capacity(n);
    let mut pps = Vec::with_capacity(n);
    for i in 0..n {
        let (z, y) = blob_opening(blobs[i], &cs[i], &roots)?;
        zs.push(z);
        ys.push(y);
        cps.push(on_curve(&cs[i])?);
        pps.push(on_curve(&pis[i])?);
    }
    let mut h = Sha256::new()
        .chain_update(b"RCKZGBATCH___V1_")
        .chain_update((FIELD_ELEMENTS_PER_BLOB as u64).to_be_bytes())
        .chain_update((n as u64).to_be_bytes());
    for i in 0..n {
        h = h.chain_update(cs[i]).chain_update(be(zs[i])).chain_update(be(ys[i])).chain_update(pis[i]);
    }
    let rb: [u8; 32] = h.finalize().into();
    let mut wide = [0u8; 64];
    for i in 0..32 {
        wide[i] = rb[31 - i];
    }
    let r = Scalar::from_bytes_wide(&wide);
    // by the curve host calls where the net has them: two MSMs and one 2-pair pairing check
    {
        let (mut a, mut b) = (Vec::with_capacity(n), Vec::with_capacity(2 * n + 1));
        let (mut ri, mut sum_ry) = (Scalar::ONE, Scalar::ZERO);
        for i in 0..n {
            a.push((pps[i], ri));
            b.push((cps[i], -ri));
            b.push((pps[i], -(ri * zs[i])));
            sum_ry += ri * ys[i];
            ri *= r;
        }
        b.push((G1Affine::generator(), sum_ry));
        if let Some(res) = calls::check(&a, &b) {
            return res;
        }
    }
    in_subgroup(&cps)?; // no calls: the subgroup checks call 103 would have made
    in_subgroup(&pps)?;
    let mut proof_lincomb = G1Projective::identity();
    let mut rhs = G1Projective::identity();
    let mut ri = Scalar::ONE;
    for i in 0..n {
        let c_minus_y = G1Projective::from(cps[i]) - G1Affine::generator() * ys[i];
        proof_lincomb += pps[i] * ri;
        rhs += c_minus_y * ri + pps[i] * (ri * zs[i]);
        ri *= r;
    }
    // e(proof_lincomb, [s]G2) * e(rhs, -G2) == 1
    let s = G2Affine::from_compressed(&SETUP_G2_S).unwrap();
    let g2_neg = -G2Affine::generator();
    let ml = multi_miller_loop(&[
        (&G1Affine::from(proof_lincomb), &G2Prepared::from(s)),
        (&G1Affine::from(rhs), &G2Prepared::from(g2_neg)),
    ]);
    Ok(ml.final_exponentiation() == Gt::identity())
}

/// The KZG pairing check e(A, [s]G2) * e(B, G2) == 1 by the proposed curve host calls (103, 104;
/// `crates/curve-calls`), A and B each a sum of scalar multiples of G1 points: two calls to 103
/// and one to 104, no G2 multiplication. None where the net does not have the calls (the
/// caller then does the check in PVM code, as before); a malformed answer is a bad point.
mod calls {
    use super::*;
    use curve_calls::Answer;

    /// [s]G2 and G2 in EIP-2537's encoding (x_c0 || x_c1 || y_c0 || y_c1, 64 octets each);
    /// tested against SETUP_G2_S and the generator.
    pub const S_G2: [u8; 256] = hex256(*b"00000000000000000000000000000000185cbfee53492714734429b7b38608e23926c911cceceac9a36851477ba4c60b087041de621000edc98edada20c1def20000000000000000000000000000000015bfd7dd8cdeb128843bc287230af38926187075cbfbefa81009a2ce615ac53d2914e5870cb452d2afaaab24f3499f7200000000000000000000000000000000014353bdb96b626dd7d5ee8599d1fca2131569490e28de18e82451a496a9c9794ce26d105941f383ee689bfbbb832a99000000000000000000000000000000001666c54b0a32529503432fcae0181b4bef79de09fc63671fda5ed1ba9bfa07899495346f3d7ac9cd23048ef30d0a154f");
    pub const G2: [u8; 256] = hex256(*b"00000000000000000000000000000000024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb80000000000000000000000000000000013e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e000000000000000000000000000000000ce5d527727d6e118cc9cdc6da2e351aadfd9baa8cbdd3a76d429a695160d12c923ac9cc3baca289e193548608b82801000000000000000000000000000000000606c4a02ea734cc32acd2b02bc28b99cb3e287e85a763af267492ab572e99ab3f370d275cec1da1aaa9075ff05f79be");

    const fn hex256(h: [u8; 512]) -> [u8; 256] {
        const fn nib(c: u8) -> u8 {
            match c { b'0'..=b'9' => c - b'0', b'a'..=b'f' => c - b'a' + 10, _ => panic!("hex") }
        }
        let mut out = [0u8; 256];
        let mut i = 0;
        while i < 256 {
            out[i] = nib(h[2 * i]) << 4 | nib(h[2 * i + 1]);
            i += 1;
        }
        out
    }

    /// G1 in EIP-2537's encoding: x || y, 64 octets each (top 16 zero); infinity all zero.
    pub fn g1(p: &G1Affine) -> [u8; 128] {
        let mut o = [0u8; 128];
        if !bool::from(p.is_identity()) {
            let u = p.to_uncompressed(); // x || y, 48 octets each, no flags for a finite point
            o[16..64].copy_from_slice(&u[..48]);
            o[80..].copy_from_slice(&u[48..]);
        }
        o
    }

    /// Could the calls be here? False once 103 or 104 answered WHAT in this invocation.
    pub fn may_have() -> bool {
        let u = curve_calls::used();
        u[2] != Some(false) && u[3] != Some(false)
    }

    fn msm(terms: &[(G1Affine, Scalar)]) -> Answer<[u8; 128]> {
        let mut input = Vec::with_capacity(160 * terms.len());
        for (p, k) in terms {
            input.extend_from_slice(&g1(p));
            input.extend_from_slice(&be(*k));
        }
        curve_calls::bls12_381_g1_msm(&input)
    }

    pub fn check(a: &[(G1Affine, Scalar)], b: &[(G1Affine, Scalar)]) -> Option<Result<bool, Error>> {
        if !may_have() {
            return None;
        }
        // A of one point times one is that point: no call needed for it (call 103 checks its
        // subgroup in B, where every point of A also appears)
        let ma = match a { [(p, k)] if *k == Scalar::ONE => Answer::Value(g1(p)), _ => msm(a) };
        let (sa, sb) = match (ma, msm(b)) {
            (Answer::Value(x), Answer::Value(y)) => (x, y),
            (Answer::Absent, _) | (_, Answer::Absent) => return None,
            _ => return Some(Err(Error::BadPoint)),
        };
        let input = [&sa[..], &S_G2[..], &sb[..], &G2[..]].concat();
        match curve_calls::bls12_381_pairing_check(&input) {
            Answer::Value(holds) => Some(Ok(holds)),
            Answer::Absent => None,
            Answer::Malformed | Answer::Broken => Some(Err(Error::BadPoint)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curve_calls::stand_in::{set_modes, Mode};
    extern crate std;
    use std::sync::Mutex;
    // the calls' detection state is global, as in a refine: one such test at a time
    static SERIAL: Mutex<()> = Mutex::new(());

    // the calls' failures refuse, never hold: HUH from either call, or WHAT after an answer
    #[test]
    fn a_failing_call_refuses() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let g = G1Affine::generator();
        let o = (g, Scalar::ZERO);
        for (what, modes) in [("103 HUH", [Mode::Present, Mode::Present, Mode::Huh, Mode::Present]),
                              ("104 HUH", [Mode::Present, Mode::Present, Mode::Present, Mode::Huh])] {
            set_modes(modes);
            assert_eq!(calls::check(&[o], &[o]), Some(Err(Error::BadPoint)), "{what}");
        }
        // a check calls 103 twice (A, then B): WHAT after its first answer refuses that check
        set_modes([Mode::Present, Mode::Present, Mode::WhatAfterFirst, Mode::Present]);
        assert_eq!(calls::check(&[o], &[o]), Some(Err(Error::BadPoint)), "103 WHAT after an answer");
        // and 104 once: the first check holds, the next is refused
        set_modes([Mode::Present, Mode::Present, Mode::Present, Mode::WhatAfterFirst]);
        assert_eq!(calls::check(&[o], &[o]), Some(Ok(true)), "104: the first check");
        assert_eq!(calls::check(&[o], &[o]), Some(Err(Error::BadPoint)), "104 WHAT after an answer: refused");
    }

    // an on-curve point outside G1, as C or as pi, is refused from a fresh state whether the
    // calls are present, absent (the probe says WHAT, then PVM code checks), or only 103 is
    #[test]
    fn a_point_outside_g1_is_refused_in_every_mode() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        // x = 0 is on y^2 = x^3 + 4 with y = 2: of order 3, outside G1; compressed with the
        // compression flag and y's sign bit for 2 (the smaller root): 0x80 then zeros
        let mut t = [0u8; 48];
        t[0] = 0x80;
        let tp = G1Affine::from_compressed_unchecked(&t).unwrap();
        assert!(!bool::from(tp.is_torsion_free()), "the test point is outside G1");
        let good = G1Affine::generator().to_compressed();
        let (z, y) = ([0u8; 32], [0u8; 32]);
        for modes in [[Mode::Present; 4], [Mode::Absent; 4], [Mode::Present, Mode::Present, Mode::Present, Mode::Absent]] {
            for (what, c, pi) in [("C", t, good), ("pi", good, t)] {
                set_modes(modes);
                assert_eq!(verify_kzg_proof(&c, &z, &y, &pi), Err(Error::BadPoint), "{what} outside G1, {modes:?}");
            }
        }
    }

    #[test]
    fn the_g2_constants_are_eip_2537s() {
        let s = G2Affine::from_compressed(&SETUP_G2_S).unwrap().to_uncompressed();
        let g = G2Affine::generator().to_uncompressed();
        for (name, k, u) in [("[s]G2", calls::S_G2, s), ("G2", calls::G2, g)] {
            // zkcrypto: x_c1 || x_c0 || y_c1 || y_c0, 48 octets each
            for (j, i) in [1usize, 0, 3, 2].iter().enumerate() {
                assert_eq!(&k[64 * j..64 * j + 16], &[0u8; 16], "{name}: top octets");
                assert_eq!(&k[64 * j + 16..64 * j + 64], &u[48 * i..48 * i + 48], "{name}: field element {j}");
            }
        }
    }

    #[test]
    fn the_calls_and_pvm_code_agree() {
        let _g = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        // e(A, [s]G2) * e(B, G2) with A = B = O holds; with A = G1 it does not. Without the
        // calls, check gives None and the caller's PVM code decides (c-kzg-4844's vectors,
        // spikes/kzg-gas/host, run both ways)
        let g = G1Affine::generator();
        for modes in [[Mode::Present; 4], [Mode::Absent; 4]] {
            set_modes(modes);
            let present = modes[2] == Mode::Present;
            let r = calls::check(&[(g, Scalar::ZERO)], &[(g, Scalar::ZERO)]);
            assert_eq!(r, if present { Some(Ok(true)) } else { None }, "{modes:?}: O and O hold");
            set_modes(modes);
            let r = calls::check(&[(g, Scalar::ONE)], &[(g, Scalar::ZERO)]);
            assert_eq!(r, if present { Some(Ok(false)) } else { None }, "{modes:?}: e(G, [s]G2) is not one");
        }
    }
}
