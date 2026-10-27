//! The proposed curve host calls (indices 101-105) for JAM services, with EIP-196/197 and
//! EIP-2537 encodings (105: EIP-196's over Grumpkin): the MSM writes its sum, the pairing check
//! answers in φ'8.
//!
//! - **Detection:** each call's first use is its probe. `WHAT` (an unknown host call: a net
//!   without the calls, or before their activation slot) marks it absent for the rest of the
//!   invocation, and the caller does that work in PVM code; later uses do not call again.
//!   Availability is fixed for an invocation, so `WHAT` after a call has answered is
//!   [Answer::Broken], never a fallback (fail closed).
//! - **`HUH`** marks the call available and is [Answer::Malformed].
//!
//! Built for anything but the PVM, the calls are arkworks over the same bytes ([stand_in]),
//! with each call's answers set per test.
#![no_std]

extern crate alloc;

use core::sync::atomic::{AtomicU8, Ordering};

/// The host's result codes (GP: WHAT = 2^64 - 2, HUH = 2^64 - 9).
pub const OK: u64 = 0;
pub const WHAT: u64 = u64::MAX - 1;
pub const HUH: u64 = u64::MAX - 8;

/// What a call gave.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer<T> {
    /// The call's result.
    Value(T),
    /// The call is not on this net (or not yet active): do the work in PVM code.
    Absent,
    /// The input is malformed (HUH).
    Malformed,
    /// WHAT after the call had answered: a broken host. Refuse.
    Broken,
}

const UNKNOWN: u8 = 0;
const AVAILABLE: u8 = 1;
const ABSENT: u8 = 2;
/// How many calls: 101..105.
pub const CALLS: usize = 5;
// per call, 101..105: unknown until its first use, then available or absent
static STATE: [AtomicU8; CALLS] = [const { AtomicU8::new(UNKNOWN) }; CALLS];

/// Which calls this invocation used: for 101..105, Some(true) the host call, Some(false) PVM
/// code, None not used yet.
pub fn used() -> [Option<bool>; CALLS] {
    core::array::from_fn(|i| match STATE[i].load(Ordering::Relaxed) { AVAILABLE => Some(true), ABSENT => Some(false), _ => None })
}

// one call through the state machine: [raw] makes the call and gives (φ'7, its value)
fn through<T>(index: usize, raw: impl FnOnce() -> (u64, Option<T>)) -> Answer<T> {
    let s = &STATE[index];
    if s.load(Ordering::Relaxed) == ABSENT {
        return Answer::Absent;
    }
    match raw() {
        (OK, Some(v)) => { s.store(AVAILABLE, Ordering::Relaxed); Answer::Value(v) }
        (WHAT, _) if s.load(Ordering::Relaxed) == AVAILABLE => Answer::Broken,
        (WHAT, _) => { s.store(ABSENT, Ordering::Relaxed); Answer::Absent }
        _ => { s.store(AVAILABLE, Ordering::Relaxed); Answer::Malformed }
    }
}

/// 101: n >= 1 items of (G1 64, scalar 32), EIP-196: the sum.
pub fn bn254_g1_msm(input: &[u8]) -> Answer<[u8; 64]> {
    through(0, || { let mut o = [0u8; 64]; let r = host::bn254_g1_msm(input, &mut o); (r, Some(o)) })
}
/// 102: k items of (G1 64, G2 128), EIP-197: does the product of the pairings equal one?
pub fn bn254_pairing_check(input: &[u8]) -> Answer<bool> {
    through(1, || { let (r, h) = host::bn254_pairing_check(input); (r, Some(h == 1)) })
}
/// 103: n >= 1 items of (G1 128, scalar 32), EIP-2537: the sum.
pub fn bls12_381_g1_msm(input: &[u8]) -> Answer<[u8; 128]> {
    through(2, || { let mut o = [0u8; 128]; let r = host::bls12_381_g1_msm(input, &mut o); (r, Some(o)) })
}
/// 104: k items of (G1 128, G2 256), EIP-2537: does the product of the pairings equal one?
pub fn bls12_381_pairing_check(input: &[u8]) -> Answer<bool> {
    through(3, || { let (r, h) = host::bls12_381_pairing_check(input); (r, Some(h == 1)) })
}
/// 105: n >= 1 items of (Grumpkin point 64, scalar 32), EIP-196's encoding over Grumpkin's
/// field: the sum.
pub fn grumpkin_g1_msm(input: &[u8]) -> Answer<[u8; 64]> {
    through(4, || { let mut o = [0u8; 64]; let r = host::grumpkin_g1_msm(input, &mut o); (r, Some(o)) })
}

#[cfg(target_env = "polkavm")]
mod host {
    #[polkavm_derive::polkavm_import]
    extern "C" {
        #[polkavm_import(index = 101)]
        fn curve_bn254_g1_msm(input: *const u8, len: u64, out: *mut u8) -> u64;
        #[polkavm_import(index = 102)]
        fn curve_bn254_pairing_check(input: *const u8, len: u64) -> (u64, u64);
        #[polkavm_import(index = 103)]
        fn curve_bls12_381_g1_msm(input: *const u8, len: u64, out: *mut u8) -> u64;
        #[polkavm_import(index = 104)]
        fn curve_bls12_381_pairing_check(input: *const u8, len: u64) -> (u64, u64);
        #[polkavm_import(index = 105)]
        fn curve_grumpkin_g1_msm(input: *const u8, len: u64, out: *mut u8) -> u64;
    }
    pub fn bn254_g1_msm(input: &[u8], out: &mut [u8; 64]) -> u64 {
        unsafe { curve_bn254_g1_msm(input.as_ptr(), input.len() as u64, out.as_mut_ptr()) }
    }
    pub fn bn254_pairing_check(input: &[u8]) -> (u64, u64) {
        unsafe { curve_bn254_pairing_check(input.as_ptr(), input.len() as u64) }
    }
    pub fn bls12_381_g1_msm(input: &[u8], out: &mut [u8; 128]) -> u64 {
        unsafe { curve_bls12_381_g1_msm(input.as_ptr(), input.len() as u64, out.as_mut_ptr()) }
    }
    pub fn bls12_381_pairing_check(input: &[u8]) -> (u64, u64) {
        unsafe { curve_bls12_381_pairing_check(input.as_ptr(), input.len() as u64) }
    }
    pub fn grumpkin_g1_msm(input: &[u8], out: &mut [u8; 64]) -> u64 {
        unsafe { curve_grumpkin_g1_msm(input.as_ptr(), input.len() as u64, out.as_mut_ptr()) }
    }
}

#[cfg(not(target_env = "polkavm"))]
mod host {
    pub use crate::stand_in::{bls12_381_g1_msm, bls12_381_pairing_check, bn254_g1_msm, bn254_pairing_check, grumpkin_g1_msm};
}

/// The calls natively, for tests: arkworks over the same bytes, as a host with the calls
/// computes them, each call answering as its [Mode] says.
#[cfg(not(target_env = "polkavm"))]
pub mod stand_in {
    use super::{CALLS, HUH, OK, WHAT};
    use alloc::vec::Vec;
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
    use ark_ff::{BigInteger, One, PrimeField, Zero};
    use core::sync::atomic::{AtomicU8, Ordering};

    /// How one call answers: as a net with it, without it, with it only once (then WHAT: a
    /// broken host), or always HUH.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Mode { Present, Absent, WhatAfterFirst, Huh }
    const MODES: [Mode; 4] = [Mode::Present, Mode::Absent, Mode::WhatAfterFirst, Mode::Huh];
    static MODE: [AtomicU8; CALLS] = [const { AtomicU8::new(0) }; CALLS];
    static ANSWERED: [AtomicU8; CALLS] = [const { AtomicU8::new(0) }; CALLS];

    /// Play a net where calls 101, 102, .. answer as [modes] says (calls past its length are
    /// present), and forget what was detected.
    pub fn set_modes<const N: usize>(modes: [Mode; N]) {
        for i in 0..CALLS {
            let m = modes.get(i).copied().unwrap_or(Mode::Present);
            MODE[i].store(MODES.iter().position(|x| *x == m).unwrap() as u8, Ordering::Relaxed);
            ANSWERED[i].store(0, Ordering::Relaxed);
            super::STATE[i].store(super::UNKNOWN, Ordering::Relaxed);
        }
    }
    /// Every call absent (true) or present (false).
    pub fn set_absent(absent: bool) {
        set_modes([if absent { Mode::Absent } else { Mode::Present }; CALLS])
    }
    // WHAT or HUH by the mode, else None: answer
    fn by_mode(i: usize) -> Option<u64> {
        match MODES[MODE[i].load(Ordering::Relaxed) as usize] {
            Mode::Present => None,
            Mode::Absent => Some(WHAT),
            Mode::Huh => Some(HUH),
            Mode::WhatAfterFirst => (ANSWERED[i].fetch_add(1, Ordering::Relaxed) > 0).then_some(WHAT),
        }
    }

    mod bn {
        use super::*;
        use ark_bn254::{Fq, Fq2, G1Affine, G2Affine};
        fn fq(b: &[u8]) -> Option<Fq> {
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
    }
    mod bls {
        use super::*;
        use ark_bls12_381::{Fq, Fq2, G1Affine, G2Affine};
        fn fq(b: &[u8]) -> Option<Fq> {
            if b[..16].iter().any(|&x| x != 0) { return None; }
            let v = Fq::from_be_bytes_mod_order(&b[16..64]);
            (v.into_bigint().to_bytes_be() == b[16..64]).then_some(v)
        }
        pub fn g1(b: &[u8]) -> Option<G1Affine> {
            let (x, y) = (fq(&b[..64])?, fq(&b[64..128])?);
            if x.is_zero() && y.is_zero() { return Some(G1Affine::identity()); }
            let p = G1Affine::new_unchecked(x, y);
            (p.is_on_curve() && p.is_in_correct_subgroup_assuming_on_curve()).then_some(p)
        }
        pub fn g2(b: &[u8]) -> Option<G2Affine> {
            let x = Fq2::new(fq(&b[..64])?, fq(&b[64..128])?);
            let y = Fq2::new(fq(&b[128..192])?, fq(&b[192..256])?);
            if x.is_zero() && y.is_zero() { return Some(G2Affine::identity()); }
            let p = G2Affine::new_unchecked(x, y);
            (p.is_on_curve() && p.is_in_correct_subgroup_assuming_on_curve()).then_some(p)
        }
    }

    pub fn bn254_g1_msm(input: &[u8], out: &mut [u8; 64]) -> u64 {
        if let Some(r) = by_mode(0) { return r; }
        if input.is_empty() || input.len() % 96 != 0 { return HUH; }
        let (mut b, mut s) = (Vec::new(), Vec::new());
        for item in input.chunks_exact(96) {
            let Some(p) = bn::g1(&item[..64]) else { return HUH };
            b.push(p);
            s.push(ark_bn254::Fr::from_be_bytes_mod_order(&item[64..]));
        }
        let r = ark_bn254::G1Projective::msm_unchecked(&b, &s).into_affine();
        *out = [0; 64];
        if let Some((x, y)) = r.xy() {
            out[..32].copy_from_slice(&x.into_bigint().to_bytes_be());
            out[32..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        OK
    }
    pub fn bn254_pairing_check(input: &[u8]) -> (u64, u64) {
        if let Some(r) = by_mode(1) { return (r, input.len() as u64); }
        if input.len() % 192 != 0 { return (HUH, input.len() as u64); }
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for item in input.chunks_exact(192) {
            match (bn::g1(&item[..64]), bn::g2(&item[64..])) {
                (Some(p), Some(q)) => { a.push(p); b.push(q); }
                _ => return (HUH, input.len() as u64),
            }
        }
        (OK, ark_bn254::Bn254::multi_pairing(a, b).0.is_one() as u64)
    }
    pub fn bls12_381_g1_msm(input: &[u8], out: &mut [u8; 128]) -> u64 {
        if let Some(r) = by_mode(2) { return r; }
        if input.is_empty() || input.len() % 160 != 0 { return HUH; }
        let (mut b, mut s) = (Vec::new(), Vec::new());
        for item in input.chunks_exact(160) {
            let Some(p) = bls::g1(&item[..128]) else { return HUH };
            b.push(p);
            s.push(ark_bls12_381::Fr::from_be_bytes_mod_order(&item[128..]));
        }
        let r = ark_bls12_381::G1Projective::msm_unchecked(&b, &s).into_affine();
        *out = [0; 128];
        if let Some((x, y)) = r.xy() {
            out[16..64].copy_from_slice(&x.into_bigint().to_bytes_be());
            out[80..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        OK
    }
    pub fn bls12_381_pairing_check(input: &[u8]) -> (u64, u64) {
        if let Some(r) = by_mode(3) { return (r, input.len() as u64); }
        if input.len() % 384 != 0 { return (HUH, input.len() as u64); }
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for item in input.chunks_exact(384) {
            match (bls::g1(&item[..128]), bls::g2(&item[128..])) {
                (Some(p), Some(q)) => { a.push(p); b.push(q); }
                _ => return (HUH, input.len() as u64),
            }
        }
        (OK, ark_bls12_381::Bls12_381::multi_pairing(a, b).0.is_one() as u64)
    }
    pub fn grumpkin_g1_msm(input: &[u8], out: &mut [u8; 64]) -> u64 {
        use ark_grumpkin::{Affine, Fq};
        if let Some(r) = by_mode(4) { return r; }
        if input.is_empty() || input.len() % 96 != 0 { return HUH; }
        let fq = |b: &[u8]| { let v = Fq::from_be_bytes_mod_order(b); (v.into_bigint().to_bytes_be() == b).then_some(v) };
        let (mut b, mut s) = (Vec::new(), Vec::new());
        for item in input.chunks_exact(96) {
            let (Some(x), Some(y)) = (fq(&item[..32]), fq(&item[32..64])) else { return HUH };
            let p = if x.is_zero() && y.is_zero() { Affine::identity() } else { Affine::new_unchecked(x, y) };
            if !p.is_on_curve() { return HUH; }
            b.push(p);
            s.push(ark_grumpkin::Fr::from_be_bytes_mod_order(&item[64..]));
        }
        let r = ark_grumpkin::Projective::msm_unchecked(&b, &s).into_affine();
        *out = [0; 64];
        if let Some((x, y)) = r.xy() {
            out[..32].copy_from_slice(&x.into_bigint().to_bytes_be());
            out[32..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        OK
    }
}
