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
//! The calls themselves, their probe state and their native stand-in are `crates/curve-calls`.

use crate::{affine, projective, FieldHooks};
use alloc::vec::Vec;
use ark_bn254_ext::{g1, g2, Bn254, CurveHooks};
use ark_ec::{pairing::Pairing, short_weierstrass::Affine, CurveConfig};
use ark_ff::{BigInteger, MontFp, One, PrimeField};
use curve_calls::Answer;

type Fq = ark_bn254::Fq;
type TargetField<H> = <Bn254<H> as Pairing>::TargetField;

/// Use as `verify::<HostCallHooks>(..)`.
#[derive(Default)]
pub struct HostCallHooks;

/// Which calls this invocation used: (101, 102), each Some(true) for the host call,
/// Some(false) for PVM code, None if not used yet.
pub fn used() -> (Option<bool>, Option<bool>) {
    let u = curve_calls::used();
    (u[0], u[1])
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
        if used().0 != Some(false) && !bases.is_empty() && bases.len() == scalars.len() {
            let mut input = Vec::with_capacity(96 * bases.len());
            for (p, s) in bases.iter().zip(scalars) {
                enc_g1(&mut input, p);
                input.extend_from_slice(&s.into_bigint().to_bytes_be());
            }
            match curve_calls::bn254_g1_msm(&input) {
                Answer::Value(out) => return dec_g1::<g1::Config<Self>>(&out).map(Into::into),
                Answer::Absent => {}
                Answer::Malformed | Answer::Broken => return Err(()),
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
        if used().1 != Some(false) && a.len() == b.len() {
            let mut input = Vec::with_capacity(192 * a.len());
            for (p, q) in a.iter().zip(&b) {
                enc_g1(&mut input, &p.0);
                enc_g2(&mut input, &q.0);
            }
            match curve_calls::bn254_pairing_check(&input) {
                Answer::Value(holds) => return Ok(verdict(holds)),
                Answer::Absent => {}
                // HUH or a broken host: Err, whose zero the pass-through final exponentiation
                // keeps "does not hold" (the call is marked available either way)
                Answer::Malformed | Answer::Broken => return Err(()),
            }
        }
        // PVM code: the true Miller loop, through FieldHooks (the same fields, another config)
        let a = a.into_iter().map(|p| <Bn254<FieldHooks> as Pairing>::G1Prepared::from(affine(&p.0)));
        let b = b.into_iter().map(|q| <Bn254<FieldHooks> as Pairing>::G2Prepared::from(affine(&q.0)));
        <FieldHooks as CurveHooks>::bn254_multi_miller_loop(a, b)
    }

    fn bn254_final_exponentiation(target: TargetField<Self>) -> Result<TargetField<Self>, ()> {
        if used().1 == Some(true) {
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
