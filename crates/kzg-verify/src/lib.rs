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

fn point(b: &[u8; 48]) -> Result<G1Affine, Error> {
    Option::from(G1Affine::from_compressed(b)).ok_or(Error::BadPoint)
}

/// c-kzg-4844's verify_kzg_proof: Ok(true) if C opens to y at z.
pub fn verify_kzg_proof(c: &[u8; 48], z: &[u8; 32], y: &[u8; 32], pi: &[u8; 48]) -> Result<bool, Error> {
    let (c, pi) = (point(c)?, point(pi)?);
    let (z, y) = (scalar(z)?, scalar(y)?);
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
    point(c)?;
    let z = challenge(blob, c);
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
    point(pi)?;
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
        cps.push(point(&cs[i])?);
        pps.push(point(&pis[i])?);
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
