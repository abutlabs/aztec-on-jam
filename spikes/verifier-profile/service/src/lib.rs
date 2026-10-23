//! Spike (#44): how much of an UltraHonk verification is curve arithmetic, which JIP-8's calls
//! 101–102 would replace? The verifier sends every curve operation through zkVerify's
//! `CurveHooks`. `Gauge` implements them on upstream arkworks, as the default `()` hooks do,
//! but converts points field by field instead of by encoding and decoding, and reads the
//! remaining gas (host call 0) around each one.
//!
//! Payload: [flavour u8: 0 plain, 1 zk][tag u8][hooks u8: 0 the default (), 1 Gauge]
//!          [vk_len u16 LE][vk][n_pubs u16 LE][n_pubs × 32][proof]
//!       or [2][tag][call u8: 0 bn254_g1_msm, 1 bn254_pairing_check][n u16 LE]: JIP-8's call as
//!          PVM code, end to end as `spikes/jip-bench`'s e2e runs it natively (#45). The input,
//!          n random points of the group in the EIP-196/197 encoding, is made unmetered; then
//!          decoding, every check and the result are metered. Refine outputs [tag][1][gas u64].
//! Refine outputs [tag][1 valid | 0][then u64 LE: verify's gas, then for MSM G1, scalar
//! multiplication G1, Miller loop and final exponentiation: gas, calls, points]; accumulate
//! stores it at "v:" ++ tag.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use ark_bn254_ext::{g1, g2, Bn254, CurveHooks};
use ark_ec::{
    bn::{G1Prepared as ArkG1Prepared, G2Prepared as ArkG2Prepared},
    pairing::{MillerLoopOutput, Pairing},
    short_weierstrass::{Affine, Projective, SWCurveConfig},
    CurveConfig, PrimeGroup, VariableBaseMSM,
};
use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::refine::gas;
use jam_pvm_common::{declare_service, Service};
use ultrahonk_no_std::{verify, ProofType};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Spike);
struct Spike;

// [gas, calls, points] for MSM G1, mul G1, Miller loop, final exponentiation. The PVM is
// single-threaded.
static mut METER: [[u64; 3]; 4] = [[0; 3]; 4];
const MSM: usize = 0;
const MUL: usize = 1;
const MILLER: usize = 2;
const FINAL: usize = 3;

fn metered<T>(slot: usize, points: usize, f: impl FnOnce() -> T) -> T {
    let before = gas();
    let out = f();
    let used = before - gas();
    unsafe {
        let m = &mut *core::ptr::addr_of_mut!(METER);
        m[slot][0] += used;
        m[slot][1] += 1;
        m[slot][2] += points as u64;
    }
    out
}

fn affine<E: SWCurveConfig, A: SWCurveConfig<BaseField = E::BaseField>>(p: &Affine<E>) -> Affine<A> {
    if p.infinity { Affine::identity() } else { Affine::new_unchecked(p.x, p.y) }
}
fn projective<E: SWCurveConfig, A: SWCurveConfig<BaseField = E::BaseField>>(p: &Projective<E>) -> Projective<A> {
    Projective::new_unchecked(p.x, p.y, p.z)
}

#[derive(Default)]
struct Gauge;

type ArkG1 = ark_bn254::g1::Config;
type ArkG2 = ark_bn254::g2::Config;

impl CurveHooks for Gauge {
    fn bn254_multi_miller_loop(
        g1: impl Iterator<Item = <Bn254<Self> as Pairing>::G1Prepared>,
        g2: impl Iterator<Item = <Bn254<Self> as Pairing>::G2Prepared>,
    ) -> Result<<Bn254<Self> as Pairing>::TargetField, ()> {
        let a: Vec<ArkG1Prepared<ark_bn254::Config>> = g1.map(|p| ArkG1Prepared::from(affine::<_, ArkG1>(&p.0))).collect();
        let b: Vec<_> = g2.collect();
        let n = a.len();
        Ok(metered(MILLER, n, || {
            let b: Vec<ArkG2Prepared<ark_bn254::Config>> = b.iter().map(|p| ArkG2Prepared::from(affine::<_, ArkG2>(&p.0))).collect();
            ark_bn254::Bn254::multi_miller_loop(a, b).0
        }))
    }

    fn bn254_final_exponentiation(
        target: <Bn254<Self> as Pairing>::TargetField,
    ) -> Result<<Bn254<Self> as Pairing>::TargetField, ()> {
        metered(FINAL, 1, || ark_bn254::Bn254::final_exponentiation(MillerLoopOutput(target)).map(|o| o.0).ok_or(()))
    }

    fn bn254_msm_g1(
        bases: &[g1::G1Affine<Self>],
        scalars: &[<g1::Config<Self> as CurveConfig>::ScalarField],
    ) -> Result<g1::G1Projective<Self>, ()> {
        metered(MSM, bases.len(), || {
            let b: Vec<Affine<ArkG1>> = bases.iter().map(affine).collect();
            <Projective<ArkG1> as VariableBaseMSM>::msm(&b, scalars).map(|r| projective(&r)).map_err(|_| ())
        })
    }

    fn bn254_msm_g2(
        bases: &[g2::G2Affine<Self>],
        scalars: &[<g2::Config<Self> as CurveConfig>::ScalarField],
    ) -> Result<g2::G2Projective<Self>, ()> {
        let b: Vec<Affine<ArkG2>> = bases.iter().map(affine).collect();
        <Projective<ArkG2> as VariableBaseMSM>::msm(&b, scalars).map(|r| projective(&r)).map_err(|_| ())
    }

    fn bn254_mul_projective_g1(base: &g1::G1Projective<Self>, scalar: &[u64]) -> Result<g1::G1Projective<Self>, ()> {
        metered(MUL, 1, || Ok(projective(&ark_ec::short_weierstrass::Projective::<ArkG1>::mul_bigint(&projective(base), scalar))))
    }

    fn bn254_mul_projective_g2(base: &g2::G2Projective<Self>, scalar: &[u64]) -> Result<g2::G2Projective<Self>, ()> {
        Ok(projective(&ark_ec::short_weierstrass::Projective::<ArkG2>::mul_bigint(&projective(base), scalar)))
    }
}

mod call {
    //! The same functions as jip-bench's e2e `bn` module.
    use alloc::vec::Vec;
    use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
    use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup, PrimeGroup, VariableBaseMSM};
    use ark_ff::{BigInteger, One, PrimeField, Zero};
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
    fn put(o: &mut Vec<u8>, f: Fq) { o.extend_from_slice(&f.into_bigint().to_bytes_be()); }
    pub fn msm(input: &[u8]) -> Option<Vec<u8>> {
        if input.is_empty() || input.len() % 96 != 0 { return None; }
        let mut bases = Vec::new();
        let mut scalars = Vec::new();
        for item in input.chunks_exact(96) {
            bases.push(g1(&item[..64])?);
            scalars.push(Fr::from_be_bytes_mod_order(&item[64..]));
        }
        let r = G1Projective::msm(&bases, &scalars).ok()?.into_affine();
        let mut o = Vec::new();
        if let Some((x, y)) = r.xy() { put(&mut o, x); put(&mut o, y); }
        Some(o)
    }
    pub fn pairing(input: &[u8]) -> Option<bool> {
        if input.len() % 192 != 0 { return None; }
        let mut a = Vec::new();
        let mut b = Vec::new();
        for item in input.chunks_exact(192) {
            a.push(g1(&item[..64])?);
            b.push(g2(&item[64..])?);
        }
        Some(Bn254::multi_pairing(a, b).0.is_one())
    }
    /// n items for `which`, from fixed pseudo-random scalars (a 64-bit LCG): not metered.
    pub fn input(which: u8, n: usize) -> Vec<u8> {
        let mut st = 0x2545F4914F6CDD1Du64;
        let mut fr = || { let mut v = [0u8; 32]; for c in v.chunks_mut(8) { st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); c.copy_from_slice(&st.to_be_bytes()); } Fr::from_be_bytes_mod_order(&v) };
        let mut o = Vec::new();
        for _ in 0..n {
            let p = (G1Projective::generator() * fr()).into_affine();
            let (x, y) = p.xy().unwrap();
            put(&mut o, x); put(&mut o, y);
            if which == 0 {
                o.extend_from_slice(&fr().into_bigint().to_bytes_be());
            } else {
                let q = (G2Projective::generator() * fr()).into_affine();
                let (x, y) = q.xy().unwrap();
                for f in [x.c1, x.c0, y.c1, y.c0] { put(&mut o, f); }
            }
        }
        o
    }
}

fn bench(p: &[u8]) -> Option<(bool, u64)> {
    let which = *p.get(2)?;
    let n = le16(p.get(3..)?)?;
    let input = call::input(which, n);
    let before = gas();
    let ok = match which { 0 => call::msm(&input).is_some(), _ => call::pairing(&input).is_some() };
    Some((ok, before - gas()))
}

fn le16(b: &[u8]) -> Option<usize> {
    Some(u16::from_le_bytes(b.get(..2)?.try_into().ok()?) as usize)
}

fn check(p: &[u8]) -> Option<(bool, u64)> {
    if *p.first()? == 2 { return bench(p); }
    let zk = *p.first()? == 1;
    let gauge = *p.get(2)? == 1;
    let vk_len = le16(p.get(3..)?)?;
    let vk = p.get(5..5 + vk_len)?;
    let at = 5 + vk_len;
    let n = le16(p.get(at..)?)?;
    let pubs: Vec<[u8; 32]> = p.get(at + 2..at + 2 + 32 * n)?.chunks_exact(32).map(|c| c.try_into().unwrap()).collect();
    let proof = p.get(at + 2 + 32 * n..)?;
    let proof = if zk { ProofType::ZK(proof.into()) } else { ProofType::Plain(proof.into()) };
    let before = gas();
    let ok = if gauge { verify::<Gauge>(vk, &proof, &pubs).is_ok() } else { verify::<()>(vk, &proof, &pubs).is_ok() };
    Some((ok, before - gas()))
}

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.get(1).copied().unwrap_or(0);
        let (ok, used) = check(&p).unwrap_or((false, 0));
        let mut out = alloc::vec![tag, ok as u8];
        out.extend_from_slice(&used.to_le_bytes());
        for row in unsafe { *core::ptr::addr_of!(METER) } {
            for v in row {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.into()
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            if let AccumulateItem::WorkItem(rec) = item {
                if let Ok(out) = rec.result {
                    if let Some(&tag) = out.0.first() {
                        set_storage(&[b'v', b':', tag], &out.0[1..]).ok();
                    }
                }
            }
        }
        None
    }
}
