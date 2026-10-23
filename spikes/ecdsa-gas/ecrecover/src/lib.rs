//! Ethereum's ecrecover, as the 0x01 precompile computes it: input hash (32) || v (32, 27 or 28)
//! || r (32) || s (32); output the signer's 20-byte address, keccak256(public key)[12..], or
//! nothing for an input that does not recover (geth core/vm/contracts.go, ecrecover).
#![no_std]

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use sha3::{Digest, Keccak256};

/// The signer's address, or None where the precompile returns nothing.
pub fn ecrecover(input: &[u8]) -> Option<[u8; 20]> {
    let mut buf = [0u8; 128];
    let n = input.len().min(128);
    buf[..n].copy_from_slice(&input[..n]); // the precompile right-pads short input with zeros
    let (hash, v, rs) = (&buf[..32], &buf[32..64], &buf[64..128]);
    // v must be 27 or 28, as a 32-byte big-endian word
    if v[..31].iter().any(|b| *b != 0) || !(v[31] == 27 || v[31] == 28) {
        return None;
    }
    let sig = Signature::from_slice(rs).ok()?; // r, s in [1, n)
    let mut id = RecoveryId::from_byte(v[31] - 27)?;
    // The precompile accepts any s below n (only transactions require low s); k256 recovers from
    // low s. (r, n - s) signs the same message with the nonce point mirrored, so y's parity flips.
    let sig = match sig.normalize_s() {
        Some(low) => {
            id = RecoveryId::new(!id.is_y_odd(), id.is_x_reduced());
            low
        }
        None => sig,
    };
    let key = VerifyingKey::recover_from_prehash(hash, &sig, id).ok()?;
    Some(address(&key))
}

/// keccak256 of the uncompressed public key without its 0x04 prefix, last 20 bytes.
pub fn address(key: &VerifyingKey) -> [u8; 20] {
    let point = key.to_encoded_point(false);
    let h = Keccak256::digest(&point.as_bytes()[1..]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&h[12..]);
    a
}
