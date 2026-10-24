//! zkVerify's `CurveHooks` (ark-bn254-ext v0.6.0) on upstream arkworks. The default `()` hooks
//! reach arkworks by encoding each point and decoding it again (`base_impl`). The two curve
//! types share their fields, so these move the fields across instead. At Aztec's 2^24 that
//! verifies the same proof for 156.0M gas instead of 181.1M (`spikes/verifier-profile`, #44, #48).
//! The arithmetic is arkworks' own either way.
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use ark_bn254_ext::{g1, g2, Bn254, CurveHooks};
use ark_ec::{
    bn::{G1Prepared as ArkG1Prepared, G2Prepared as ArkG2Prepared},
    pairing::{MillerLoopOutput, Pairing},
    short_weierstrass::{Affine, Projective, SWCurveConfig},
    CurveConfig, PrimeGroup, VariableBaseMSM,
};

type ArkG1 = ark_bn254::g1::Config;
type ArkG2 = ark_bn254::g2::Config;

fn affine<E: SWCurveConfig, A: SWCurveConfig<BaseField = E::BaseField>>(p: &Affine<E>) -> Affine<A> {
    if p.infinity { Affine::identity() } else { Affine::new_unchecked(p.x, p.y) }
}

fn projective<E: SWCurveConfig, A: SWCurveConfig<BaseField = E::BaseField>>(p: &Projective<E>) -> Projective<A> {
    Projective::new_unchecked(p.x, p.y, p.z)
}

/// Use as `verify::<FieldHooks>(..)`.
#[derive(Default)]
pub struct FieldHooks;

impl CurveHooks for FieldHooks {
    fn bn254_multi_miller_loop(
        g1: impl Iterator<Item = <Bn254<Self> as Pairing>::G1Prepared>,
        g2: impl Iterator<Item = <Bn254<Self> as Pairing>::G2Prepared>,
    ) -> Result<<Bn254<Self> as Pairing>::TargetField, ()> {
        let a: Vec<ArkG1Prepared<ark_bn254::Config>> = g1.map(|p| ArkG1Prepared::from(affine::<_, ArkG1>(&p.0))).collect();
        let b: Vec<ArkG2Prepared<ark_bn254::Config>> = g2.map(|p| ArkG2Prepared::from(affine::<_, ArkG2>(&p.0))).collect();
        Ok(ark_bn254::Bn254::multi_miller_loop(a, b).0)
    }

    fn bn254_final_exponentiation(target: <Bn254<Self> as Pairing>::TargetField) -> Result<<Bn254<Self> as Pairing>::TargetField, ()> {
        ark_bn254::Bn254::final_exponentiation(MillerLoopOutput(target)).map(|o| o.0).ok_or(())
    }

    fn bn254_msm_g1(bases: &[g1::G1Affine<Self>], scalars: &[<g1::Config<Self> as CurveConfig>::ScalarField]) -> Result<g1::G1Projective<Self>, ()> {
        let b: Vec<Affine<ArkG1>> = bases.iter().map(affine).collect();
        <Projective<ArkG1> as VariableBaseMSM>::msm(&b, scalars).map(|r| projective(&r)).map_err(|_| ())
    }

    fn bn254_msm_g2(bases: &[g2::G2Affine<Self>], scalars: &[<g2::Config<Self> as CurveConfig>::ScalarField]) -> Result<g2::G2Projective<Self>, ()> {
        let b: Vec<Affine<ArkG2>> = bases.iter().map(affine).collect();
        <Projective<ArkG2> as VariableBaseMSM>::msm(&b, scalars).map(|r| projective(&r)).map_err(|_| ())
    }

    fn bn254_mul_projective_g1(base: &g1::G1Projective<Self>, scalar: &[u64]) -> Result<g1::G1Projective<Self>, ()> {
        Ok(projective(&Projective::<ArkG1>::mul_bigint(&projective(base), scalar)))
    }

    fn bn254_mul_projective_g2(base: &g2::G2Projective<Self>, scalar: &[u64]) -> Result<g2::G2Projective<Self>, ()> {
        Ok(projective(&Projective::<ArkG2>::mul_bigint(&projective(base), scalar)))
    }
}
