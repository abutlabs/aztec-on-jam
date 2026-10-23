//! EIP-4844's KZG point evaluation, as Ethereum's 0x0a precompile checks it (Aztec's
//! `BlobLib.sol` calls it once per epoch proof): the commitment C opens to y at z, with proof
//! pi, against Ethereum's trusted setup. c-kzg-4844's `verify_kzg_proof`:
//!   e(pi, [s]G2 - [z]G2) == e(C - [y]G1, G2)
//! checked as one multi-Miller loop and one final exponentiation.
#![no_std]

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
    /// the input is not 192 bytes
    BadLength,
}

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
