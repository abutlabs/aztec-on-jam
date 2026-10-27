//! `HostCallHooks`: the verifier's BN254 work through the proposed curve host calls (101, 102) where the net has them
//! (#58), `FieldHooks` where it doesn't. The same blob runs on both.
//!
//! - **MSM:** `bn254_msm_g1` is call 101, its input EIP-196's (point x ‖ y, then the scalar,
//!   32 octets each, big-endian; (0, 0) is infinity).
//! - **Pairing:** call 102 is the whole check, ∏ e(a_i, b_i) = 1, while the hooks split the
//!   pairing into a Miller loop and a final exponentiation. ark-bn254-ext's prepared points
//!   keep their affine points, so the Miller-loop hook calls 102 and returns a verdict in place
//!   of a Miller-loop value: `Fq12::ONE` if the check holds, otherwise e(G1, G2) ([FAILED]),
//!   which no final exponentiation turns into one. The final exponentiation passes its input
//!   through (one stays one). `multi_pairing(..).is_one()` is then exactly call 102's answer. The verifier uses the pairing only that way (ultrahonk_verifier
//!   src/lib.rs, `verify_shplemini`), so the value is a verdict, not a pairing.
//! - **Detection:** each call's first use is its probe. `WHAT` (an unknown host call: no curve calls,
//!   or before its activation slot) marks it unavailable for the rest of the invocation, and
//!   `FieldHooks` does that work in PVM code, at 1,000 gas for the probe. Availability is fixed
//!   for an invocation, so a `WHAT` after the call has answered is an error, never a fallback.
//! - **`HUH`** (a malformed input, which the verifier's own parsing should already refuse)
//!   marks the call available and is the hook's `Err`: the proof is rejected.
//!
//! Built for anything but the PVM, the two calls are arkworks over the same bytes (`host`
//! below), so the encodings are tested natively.

use crate::{affine, projective, FieldHooks};
use alloc::vec::Vec;
use ark_bn254_ext::{g1, g2, Bn254, CurveHooks};
use ark_ec::{pairing::Pairing, short_weierstrass::Affine, CurveConfig};
use ark_ff::{BigInteger, MontFp, One, PrimeField};
use core::sync::atomic::{AtomicU8, Ordering};

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
                // availability is fixed for an invocation: WHAT after an answer is a broken host
                WHAT if MSM.load(Ordering::Relaxed) == AVAILABLE => return Err(()),
                WHAT => MSM.store(ABSENT, Ordering::Relaxed),
                _ => {
                    MSM.store(AVAILABLE, Ordering::Relaxed); // HUH: the call exists
                    return Err(());
                }
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
                    return Ok(verdict(holds == 1));
                }
                (WHAT, _) if PAIRING.load(Ordering::Relaxed) == AVAILABLE => return Err(()),
                (WHAT, _) => PAIRING.store(ABSENT, Ordering::Relaxed),
                _ => {
                    // HUH: the call exists, so the final exponentiation passes through and
                    // the Err's zero (ark-bn254-ext's default) is "does not hold", not a trap
                    PAIRING.store(AVAILABLE, Ordering::Relaxed);
                    return Err(());
                }
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

/// e(G1, G2), the target group's generator: the verdict of a failed check. It is not one, and
/// it stays not one through a real final exponentiation (its order is r, prime to
/// (p^12 - 1)/r), so a verdict can never turn into "holds" whichever final exponentiation
/// it meets (the review of #58: a non-one value of Fp did, a^((p^12-1)/r) = 1 for a in Fp).
/// Checked against arkworks' own pairing in tests/encoding.rs.
pub const FAILED: ark_bn254::Fq12 = ark_bn254::Fq12::new(
    ark_bn254::Fq6::new(ark_bn254::Fq2::new(MontFp!("17264119758069723980713015158403419364912226240334615592005620718956030922389"), MontFp!("1300711225518851207585954685848229181392358478699795190245709208408267917898")), ark_bn254::Fq2::new(MontFp!("8894217292938489450175280157304813535227569267786222825147475294561798790624"), MontFp!("1829859855596098509359522796979920150769875799037311140071969971193843357227")), ark_bn254::Fq2::new(MontFp!("4968700049505451466697923764727215585075098085662966862137174841375779106779"), MontFp!("12814315002058128940449527172080950701976819591738376253772993495204862218736"))),
    ark_bn254::Fq6::new(ark_bn254::Fq2::new(MontFp!("4233474252585134102088637248223601499779641130562251948384759786370563844606"), MontFp!("9420544134055737381096389798327244442442230840902787283326002357297404128074")), ark_bn254::Fq2::new(MontFp!("13457906610892676317612909831857663099224588803620954529514857102808143524905"), MontFp!("5122435115068592725432309312491733755581898052459744089947319066829791570839")), ark_bn254::Fq2::new(MontFp!("8891987925005301465158626530377582234132838601606565363865129986128301774627"), MontFp!("440796048150724096437130979851431985500142692666486515369083499585648077975"))));

/// One if the check holds, else [FAILED].
fn verdict(holds: bool) -> ark_bn254::Fq12 {
    if holds { ark_bn254::Fq12::one() } else { FAILED }
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
    use core::sync::atomic::{AtomicU8, Ordering};

    /// How the stand-in answers one call: as a net with it, without it, with it but only once
    /// (then WHAT: a broken host, availability changing mid-invocation), or always HUH.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Mode { Present, Absent, WhatAfterFirst, Huh }
    const MODES: [Mode; 4] = [Mode::Present, Mode::Absent, Mode::WhatAfterFirst, Mode::Huh];
    static MSM_MODE: AtomicU8 = AtomicU8::new(0);
    static PAIRING_MODE: AtomicU8 = AtomicU8::new(0);
    static MSM_ANSWERED: AtomicU8 = AtomicU8::new(0);
    static PAIRING_ANSWERED: AtomicU8 = AtomicU8::new(0);

    /// Play a net with each call answering as [Mode] says, and forget what was detected.
    pub fn set_calls(msm: Mode, pairing: Mode) {
        let i = |m| MODES.iter().position(|x| *x == m).unwrap() as u8;
        MSM_MODE.store(i(msm), Ordering::Relaxed);
        PAIRING_MODE.store(i(pairing), Ordering::Relaxed);
        MSM_ANSWERED.store(0, Ordering::Relaxed);
        PAIRING_ANSWERED.store(0, Ordering::Relaxed);
        super::MSM.store(super::UNKNOWN, Ordering::Relaxed);
        super::PAIRING.store(super::UNKNOWN, Ordering::Relaxed);
    }
    /// Play a net without the calls (true) or with them (false).
    pub fn set_absent(absent: bool) {
        let m = if absent { Mode::Absent } else { Mode::Present };
        set_calls(m, m)
    }
    // WHAT or HUH by the mode, else None: answer
    fn by_mode(mode: &AtomicU8, answered: &AtomicU8) -> Option<u64> {
        match MODES[mode.load(Ordering::Relaxed) as usize] {
            Mode::Present => None,
            Mode::Absent => Some(WHAT),
            Mode::Huh => Some(HUH),
            Mode::WhatAfterFirst => (answered.fetch_add(1, Ordering::Relaxed) > 0).then_some(WHAT),
        }
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
        if let Some(r) = by_mode(&MSM_MODE, &MSM_ANSWERED) { return r; }
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
        if let Some(r) = by_mode(&PAIRING_MODE, &PAIRING_ANSWERED) { return (r, input.len() as u64); }
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
