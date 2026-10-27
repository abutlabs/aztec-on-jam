//! `HostCallHooks`: the verifier's BN254 work through the proposed curve host calls (101, 102) where the net has them
//! (#58), `FieldHooks` where it doesn't. The same blob runs on both.
//!
//! - **MSM:** `bn254_msm_g1` is call 101, its input EIP-196's (point x ‖ y, then the scalar,
//!   32 octets each, big-endian; (0, 0) is infinity).
//! - **Pairing:** call 102 is the whole check, ∏ e(a_i, b_i) = 1, while the hooks split the
//!   pairing into a Miller loop and a final exponentiation. ark-bn254-ext's prepared points
//!   keep their affine points, so the Miller-loop hook calls 102 and returns a verdict in place
//!   of a Miller-loop value: `Fq12::ONE` if the check holds, otherwise a non-one value
//!   distinct per call. The final exponentiation passes its input through (one stays one).
//!   `multi_pairing(..).is_one()` is then exactly call 102's answer; two failed checks never
//!   compare equal. The verifier uses the pairing only that way (ultrahonk_verifier
//!   src/lib.rs, `verify_shplemini`), so the value is a verdict, not a pairing.
//! - **Detection:** each call's first use is its probe. `WHAT` (an unknown host call: no curve calls,
//!   or before its activation slot) marks it unavailable for the rest of the invocation, and
//!   `FieldHooks` does that work in PVM code, at 1,000 gas for the probe.
//! - **`HUH`** (a malformed input, which the verifier's own parsing should already refuse) is
//!   the hook's `Err`: the proof is rejected.
//!
//! Built for anything but the PVM, the two calls are arkworks over the same bytes (`host`
//! below), so the encodings are tested natively.

use crate::{affine, projective, FieldHooks};
use alloc::vec::Vec;
use ark_bn254_ext::{g1, g2, Bn254, CurveHooks};
use ark_ec::{pairing::Pairing, short_weierstrass::Affine, CurveConfig};
use ark_ff::{BigInteger, One, PrimeField, Zero};
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};

type Fq = ark_bn254::Fq;
type TargetField<H> = <Bn254<H> as Pairing>::TargetField;

/// The host's result codes (GP: WHAT = 2^64 - 2, HUH = 2^64 - 9).
pub const OK: u64 = 0;
pub const WHAT: u64 = u64::MAX - 1;
// the hooks treat any answer but OK and WHAT as HUH; only the native stand-in names it
#[cfg_attr(target_env = "polkavm", allow(dead_code))]
pub const HUH: u64 = u64::MAX - 8;

/// Use as `verify::<HostCallHooks>(..)`.
#[derive(Default)]
pub struct HostCallHooks;

// per call: unknown until its first use, then available or not, for this invocation
const UNKNOWN: u8 = 0;
const AVAILABLE: u8 = 1;
const ABSENT: u8 = 2;
static MSM: AtomicU8 = AtomicU8::new(UNKNOWN);
static PAIRING: AtomicU8 = AtomicU8::new(UNKNOWN);
static FAILED_CHECKS: AtomicU64 = AtomicU64::new(0);

/// Which hooks did this invocation's calls use? (101, 102), each Some(true) for the host call,
/// Some(false) for PVM code, None if not used yet.
pub fn used() -> (Option<bool>, Option<bool>) {
    let f = |s: &AtomicU8| match s.load(Ordering::Relaxed) { AVAILABLE => Some(true), ABSENT => Some(false), _ => None };
    (f(&MSM), f(&PAIRING))
}

fn put_fq(out: &mut Vec<u8>, f: &Fq) {
    out.extend_from_slice(&f.into_bigint().to_bytes_be());
}

/// G1 as EIP-196's 64 octets.
pub fn enc_g1<C: ark_ec::short_weierstrass::SWCurveConfig<BaseField = Fq>>(out: &mut Vec<u8>, p: &Affine<C>) {
    if p.infinity {
        out.extend_from_slice(&[0; 64]);
    } else {
        put_fq(out, &p.x);
        put_fq(out, &p.y);
    }
}

/// G2 as EIP-197's 128 octets: x_im ‖ x_re ‖ y_im ‖ y_re.
pub fn enc_g2<C: ark_ec::short_weierstrass::SWCurveConfig<BaseField = ark_bn254::Fq2>>(out: &mut Vec<u8>, p: &Affine<C>) {
    if p.infinity {
        out.extend_from_slice(&[0; 128]);
    } else {
        for f in [p.x.c1, p.x.c0, p.y.c1, p.y.c0] {
            put_fq(out, &f);
        }
    }
}

/// A host's 64-octet G1 (it wrote a point it computed, so trusted to be on the curve).
fn dec_g1<C: ark_ec::short_weierstrass::SWCurveConfig<BaseField = Fq>>(b: &[u8; 64]) -> Result<Affine<C>, ()> {
    if b.iter().all(|&x| x == 0) {
        return Ok(Affine::identity());
    }
    let fq = |s: &[u8]| {
        let v = Fq::from_be_bytes_mod_order(s);
        (v.into_bigint().to_bytes_be() == s).then_some(v).ok_or(())
    };
    let p = Affine::new_unchecked(fq(&b[..32])?, fq(&b[32..])?);
    p.is_on_curve().then_some(p).ok_or(())
}

impl CurveHooks for HostCallHooks {
    fn bn254_msm_g1(bases: &[g1::G1Affine<Self>], scalars: &[<g1::Config<Self> as CurveConfig>::ScalarField]) -> Result<g1::G1Projective<Self>, ()> {
        if MSM.load(Ordering::Relaxed) != ABSENT && !bases.is_empty() && bases.len() == scalars.len() {
            let mut input = Vec::with_capacity(96 * bases.len());
            for (p, s) in bases.iter().zip(scalars) {
                enc_g1(&mut input, p);
                input.extend_from_slice(&s.into_bigint().to_bytes_be());
            }
            let mut out = [0u8; 64];
            match host::bn254_g1_msm(&input, &mut out) {
                OK => {
                    MSM.store(AVAILABLE, Ordering::Relaxed);
                    return dec_g1::<g1::Config<Self>>(&out).map(Into::into);
                }
                WHAT => MSM.store(ABSENT, Ordering::Relaxed),
                _ => return Err(()),
            }
        }
        let b: Vec<_> = bases.iter().map(affine).collect();
        <FieldHooks as CurveHooks>::bn254_msm_g1(&b, scalars).map(|r| projective(&r))
    }

    fn bn254_multi_miller_loop(
        a: impl Iterator<Item = <Bn254<Self> as Pairing>::G1Prepared>,
        b: impl Iterator<Item = <Bn254<Self> as Pairing>::G2Prepared>,
    ) -> Result<TargetField<Self>, ()> {
        let (a, b): (Vec<_>, Vec<_>) = (a.collect(), b.collect());
        if PAIRING.load(Ordering::Relaxed) != ABSENT && a.len() == b.len() {
            let mut input = Vec::with_capacity(192 * a.len());
            for (p, q) in a.iter().zip(&b) {
                enc_g1(&mut input, &p.0);
                enc_g2(&mut input, &q.0);
            }
            match host::bn254_pairing_check(&input) {
                (OK, holds) => {
                    PAIRING.store(AVAILABLE, Ordering::Relaxed);
                    return Ok(verdict::<Self>(holds == 1));
                }
                (WHAT, _) => PAIRING.store(ABSENT, Ordering::Relaxed),
                _ => return Err(()),
            }
        }
        // PVM code: the true Miller loop, through FieldHooks (the same fields, another config)
        let a = a.into_iter().map(|p| <Bn254<FieldHooks> as Pairing>::G1Prepared::from(affine(&p.0)));
        let b = b.into_iter().map(|q| <Bn254<FieldHooks> as Pairing>::G2Prepared::from(affine(&q.0)));
        <FieldHooks as CurveHooks>::bn254_multi_miller_loop(a, b)
    }

    fn bn254_final_exponentiation(target: TargetField<Self>) -> Result<TargetField<Self>, ()> {
        if PAIRING.load(Ordering::Relaxed) == AVAILABLE {
            Ok(target) // a verdict from call 102, not a Miller-loop value
        } else {
            <FieldHooks as CurveHooks>::bn254_final_exponentiation(target)
        }
    }

    fn bn254_msm_g2(bases: &[g2::G2Affine<Self>], scalars: &[<g2::Config<Self> as CurveConfig>::ScalarField]) -> Result<g2::G2Projective<Self>, ()> {
        let b: Vec<_> = bases.iter().map(affine).collect();
        <FieldHooks as CurveHooks>::bn254_msm_g2(&b, scalars).map(|r| projective(&r))
    }

    fn bn254_mul_projective_g1(base: &g1::G1Projective<Self>, scalar: &[u64]) -> Result<g1::G1Projective<Self>, ()> {
        <FieldHooks as CurveHooks>::bn254_mul_projective_g1(&projective(base), scalar).map(|r| projective(&r))
    }

    fn bn254_mul_projective_g2(base: &g2::G2Projective<Self>, scalar: &[u64]) -> Result<g2::G2Projective<Self>, ()> {
        <FieldHooks as CurveHooks>::bn254_mul_projective_g2(&projective(base), scalar).map(|r| projective(&r))
    }
}

/// One if the check holds; otherwise a non-one value, distinct per failed check.
fn verdict<H: CurveHooks>(holds: bool) -> TargetField<H> {
    if holds {
        return TargetField::<H>::one();
    }
    let n = FAILED_CHECKS.fetch_add(1, Ordering::Relaxed);
    let mut v = TargetField::<H>::zero();
    v.c0.c0.c0 = Fq::from(n + 2);
    v
}

/// The two host calls. On the PVM, polkavm imports 101 and 102; elsewhere, arkworks over the
/// same bytes, as a host with the calls computes them (for tests), with `set_absent` to play a net
/// without the calls.
#[cfg(target_env = "polkavm")]
mod host {
    #[polkavm_derive::polkavm_import]
    extern "C" {
        #[polkavm_import(index = 101)]
        fn curve_bn254_g1_msm(input: *const u8, len: u64, out: *mut u8) -> u64;
        #[polkavm_import(index = 102)]
        fn curve_bn254_pairing_check(input: *const u8, len: u64) -> (u64, u64);
    }
    pub fn bn254_g1_msm(input: &[u8], out: &mut [u8; 64]) -> u64 {
        unsafe { curve_bn254_g1_msm(input.as_ptr(), input.len() as u64, out.as_mut_ptr()) }
    }
    pub fn bn254_pairing_check(input: &[u8]) -> (u64, u64) {
        unsafe { curve_bn254_pairing_check(input.as_ptr(), input.len() as u64) }
    }
}

#[cfg(not(target_env = "polkavm"))]
pub mod host {
    use super::{HUH, OK, WHAT};
    use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine};
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, VariableBaseMSM};
    use ark_ff::{BigInteger, One, PrimeField, Zero};
    use core::sync::atomic::{AtomicBool, Ordering};

    static ABSENT: AtomicBool = AtomicBool::new(false);
    /// Play a net without the calls (true) or with it (false), and forget what was detected.
    pub fn set_absent(absent: bool) {
        ABSENT.store(absent, Ordering::Relaxed);
        super::MSM.store(super::UNKNOWN, Ordering::Relaxed);
        super::PAIRING.store(super::UNKNOWN, Ordering::Relaxed);
    }

    fn fq(b: &[u8]) -> Option<Fq> {
        let v = Fq::from_be_bytes_mod_order(b);
        (v.into_bigint().to_bytes_be() == b).then_some(v)
    }
    fn g1(b: &[u8]) -> Option<G1Affine> {
        let (x, y) = (fq(&b[..32])?, fq(&b[32..64])?);
        if x.is_zero() && y.is_zero() { return Some(G1Affine::identity()); }
        let p = G1Affine::new_unchecked(x, y);
        p.is_on_curve().then_some(p)
    }
    fn g2(b: &[u8]) -> Option<G2Affine> {
        let x = Fq2::new(fq(&b[32..64])?, fq(&b[..32])?);
        let y = Fq2::new(fq(&b[96..128])?, fq(&b[64..96])?);
        if x.is_zero() && y.is_zero() { return Some(G2Affine::identity()); }
        let p = G2Affine::new_unchecked(x, y);
        (p.is_on_curve() && p.is_in_correct_subgroup_assuming_on_curve()).then_some(p)
    }

    pub fn bn254_g1_msm(input: &[u8], out: &mut [u8; 64]) -> u64 {
        if ABSENT.load(Ordering::Relaxed) { return WHAT; }
        if input.is_empty() || input.len() % 96 != 0 { return HUH; }
        let mut bases = alloc::vec::Vec::new();
        let mut scalars = alloc::vec::Vec::new();
        for item in input.chunks_exact(96) {
            let Some(p) = g1(&item[..64]) else { return HUH };
            bases.push(p);
            scalars.push(Fr::from_be_bytes_mod_order(&item[64..]));
        }
        let r = G1Projective::msm_unchecked(&bases, &scalars).into_affine();
        *out = [0; 64];
        if let Some((x, y)) = r.xy() {
            out[..32].copy_from_slice(&x.into_bigint().to_bytes_be());
            out[32..].copy_from_slice(&y.into_bigint().to_bytes_be());
        }
        OK
    }

    pub fn bn254_pairing_check(input: &[u8]) -> (u64, u64) {
        if ABSENT.load(Ordering::Relaxed) { return (WHAT, input.len() as u64); }
        if input.len() % 192 != 0 { return (HUH, input.len() as u64); }
        let (mut a, mut b) = (alloc::vec::Vec::new(), alloc::vec::Vec::new());
        for item in input.chunks_exact(192) {
            match (g1(&item[..64]), g2(&item[64..])) {
                (Some(p), Some(q)) => { a.push(p); b.push(q); }
                _ => return (HUH, input.len() as u64),
            }
        }
        (OK, Bn254::multi_pairing(a, b).0.is_one() as u64)
    }
}
