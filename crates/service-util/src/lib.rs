//! Shared by the privacy and public-balance services. `no_std`, allocation only for keys.
#![no_std]
extern crate alloc;
use alloc::vec::Vec;

/// `prefix ++ bytes`, the storage key for a hash or an account under a short tag.
pub fn key(prefix: &[u8], bytes: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(prefix.len() + bytes.len());
    k.extend_from_slice(prefix);
    k.extend_from_slice(bytes);
    k
}

/// The first eight bytes as a little-endian u64; shorter input is zero-padded.
pub fn le_u64(b: &[u8]) -> u64 {
    let mut x = [0u8; 8];
    let n = core::cmp::min(8, b.len());
    x[..n].copy_from_slice(&b[..n]);
    u64::from_le_bytes(x)
}

/// A fixed-width read, or None when the slice is short.
pub fn take<const N: usize>(b: &[u8], at: usize) -> Option<[u8; N]> {
    b.get(at..at + N).map(|s| s.try_into().unwrap())
}
