//! #58: the verifier's verdicts are the same whichever hooks run it: `()`, `FieldHooks`, and
//! `HostCallHooks` on a net with the curve host calls 101 and 102 or without them. Valid proofs, a tampered
//! proof, an altered public input, a wrong vk; on the bb 6 verifier (aztec-settlement's pin,
//! plain UltraHonk, 2^16 in tests/fixtures, 2^20 and 2^24 from spikes/epoch-proof/out when it is
//! there) and the bb 5 one (the privacy service's pin, its ZK transfer proof).
//!
//! One test: the hooks' detection state is global, as in a refine.

use curve_hooks::{host_calls_stand_in, host_calls_used, FieldHooks, HostCallHooks};
use curve_hooks::host_calls_stand_in::Mode;
use std::path::{Path, PathBuf};

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}
fn pubs(b: &[u8]) -> Vec<[u8; 32]> {
    b.chunks_exact(32).map(|c| c.try_into().unwrap()).collect()
}

#[derive(Clone, Copy, Debug)]
enum Uh {
    V6Plain,
    V5Zk,
}

/// The verdict under every set of hooks; all must agree. Returns it, and checks which path
/// HostCallHooks took.
fn verdicts(uh: Uh, vk: &[u8], proof: &[u8], p: &[[u8; 32]], what: &str) -> bool {
    fn run<H: ark_bn254_ext::CurveHooks + Default>(uh: Uh, vk: &[u8], proof: &[u8], p: &[[u8; 32]]) -> bool {
        match uh {
            Uh::V6Plain => uh6::verify::<H>(vk, &uh6::ProofType::Plain(proof.into()), p).is_ok(),
            Uh::V5Zk => uh5::verify::<H>(vk, &uh5::ProofType::ZK(proof.into()), p).is_ok(),
        }
    }
    let plain = run::<()>(uh, vk, proof, p);
    let field = run::<FieldHooks>(uh, vk, proof, p);
    host_calls_stand_in::set_absent(false);
    let with_calls = run::<HostCallHooks>(uh, vk, proof, p);
    let used_with = host_calls_used();
    host_calls_stand_in::set_absent(true);
    let without = run::<HostCallHooks>(uh, vk, proof, p);
    let used_without = host_calls_used();
    host_calls_stand_in::set_absent(false);
    println!("  {what:45} {uh:?}: () {plain}, FieldHooks {field}, with the calls {with_calls} {used_with:?}, without {without} {used_without:?}");
    assert_eq!((field, with_calls, without), (plain, plain, plain), "{what}: the verdicts differ");
    // wherever the verifier reached its curve work, it took the calls, or PVM code without them
    for (u, call) in [(used_with.0, true), (used_with.1, true), (used_without.0, false), (used_without.1, false)] {
        assert!(u.is_none() || u == Some(call), "{what}: wrong path {used_with:?} / {used_without:?}");
    }
    if plain {
        assert_eq!(used_with, (Some(true), Some(true)), "{what}: a valid proof must use both calls");
        assert_eq!(used_without, (Some(false), Some(false)), "{what}: and both fallbacks without them");
    }
    plain
}

fn cases(uh: Uh, name: &str, dir: &Path, wrong_vk: &[u8], bad_pubs: Option<&Path>) {
    let (vk, proof, p) = (read(&dir.join("vk")), read(&dir.join("proof")), pubs(&read(&dir.join("public_inputs"))));
    assert!(verdicts(uh, &vk, &proof, &p, &format!("{name}: valid")));
    let mut t = proof.clone();
    let mid = t.len() / 2;
    t[mid] ^= 1;
    assert!(!verdicts(uh, &vk, &t, &p, &format!("{name}: one proof bit flipped")));
    let mut t = proof.clone();
    let end = t.len() - 40; // late in the proof: the opening, read by the pairing step
    t[end] ^= 0x10;
    assert!(!verdicts(uh, &vk, &t, &p, &format!("{name}: a late proof bit flipped")));
    let mut q = p.clone();
    q[0][31] ^= 1;
    assert!(!verdicts(uh, &vk, &proof, &q, &format!("{name}: a public input altered")));
    if let Some(b) = bad_pubs {
        assert!(!verdicts(uh, &vk, &proof, &pubs(&read(b)), &format!("{name}: public_inputs.bad")));
    }
    assert!(!verdicts(uh, wrong_vk, &proof, &p, &format!("{name}: another circuit's vk")));
    // the KZG quotient, the proof's last 64 octets, is read after every challenge is drawn:
    // replaced by another valid point, everything up to the pairing passes and the pairing fails
    for (what, point) in [("the generator", g1_bytes(1, 2)), ("infinity", [0u8; 64])] {
        let mut t = proof.clone();
        let n = t.len();
        t[n - 64..].copy_from_slice(&point);
        assert!(!verdicts(uh, &vk, &t, &p, &format!("{name}: KZG quotient = {what}")));
        host_calls_stand_in::set_absent(false);
        assert!(run_with_calls(uh, &vk, &t, &p) == false && host_calls_used() == (Some(true), Some(true)),
                "{name}, KZG quotient = {what}: must fail at call 102 itself");
    }
}

/// The review of #58: a net with one call and not the other; a host whose call answers once
/// and then WHAT (availability may not change mid-invocation: fail closed); a call that
/// answers HUH (reject, never trap). Valid proofs and a proof that fails only at the pairing.
fn odd_hosts(uh: Uh, name: &str, dir: &Path) {
    let (vk, proof, p) = (read(&dir.join("vk")), read(&dir.join("proof")), pubs(&read(&dir.join("public_inputs"))));
    let mut bad = proof.clone();
    let n = bad.len();
    bad[n - 64..].copy_from_slice(&g1_bytes(1, 2));
    for (msm, pairing) in [(Mode::Present, Mode::Absent), (Mode::Absent, Mode::Present)] {
        host_calls_stand_in::set_calls(msm, pairing);
        assert!(run_with_calls(uh, &vk, &proof, &p), "{name}: valid, MSM {msm:?}, pairing {pairing:?}");
        assert_eq!(host_calls_used(), (Some(msm == Mode::Present), Some(pairing == Mode::Present)), "{name}: paths");
        host_calls_stand_in::set_calls(msm, pairing);
        assert!(!run_with_calls(uh, &vk, &bad, &p), "{name}: KZG quotient = G, MSM {msm:?}, pairing {pairing:?}");
    }
    // one answer, then WHAT: the second verification in the same invocation fails closed
    host_calls_stand_in::set_calls(Mode::WhatAfterFirst, Mode::WhatAfterFirst);
    assert!(run_with_calls(uh, &vk, &proof, &p), "{name}: the first verification, the calls answering");
    assert!(!run_with_calls(uh, &vk, &proof, &p), "{name}: then WHAT: rejected, not a silent fallback");
    // each call on its own: the other keeps answering, so only this call's guard can refuse
    for (msm, pairing) in [(Mode::WhatAfterFirst, Mode::Present), (Mode::Present, Mode::WhatAfterFirst)] {
        host_calls_stand_in::set_calls(msm, pairing);
        assert!(run_with_calls(uh, &vk, &proof, &p), "{name}: first, MSM {msm:?}, pairing {pairing:?}");
        assert!(!run_with_calls(uh, &vk, &proof, &p), "{name}: then WHAT from one call: rejected, MSM {msm:?}, pairing {pairing:?}");
    }
    host_calls_stand_in::set_calls(Mode::Present, Mode::WhatAfterFirst);
    assert!(!run_with_calls(uh, &vk, &bad, &p), "{name}: a failing pairing, then WHAT");
    assert!(!run_with_calls(uh, &vk, &bad, &p), "{name}: a failing pairing stays failing after WHAT");
    // HUH: rejected without a trap
    for (msm, pairing) in [(Mode::Huh, Mode::Present), (Mode::Present, Mode::Huh)] {
        host_calls_stand_in::set_calls(msm, pairing);
        let r = std::panic::catch_unwind(|| run_with_calls(uh, &vk, &proof, &p));
        assert_eq!(r.ok(), Some(false), "{name}: MSM {msm:?}, pairing {pairing:?}: rejected, no panic");
    }
    host_calls_stand_in::set_absent(false);
}

fn g1_bytes(x: u8, y: u8) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[31] = x;
    b[63] = y;
    b
}

fn run_with_calls(uh: Uh, vk: &[u8], proof: &[u8], p: &[[u8; 32]]) -> bool {
    match uh {
        Uh::V6Plain => uh6::verify::<HostCallHooks>(vk, &uh6::ProofType::Plain(proof.into()), p).is_ok(),
        Uh::V5Zk => uh5::verify::<HostCallHooks>(vk, &uh5::ProofType::ZK(proof.into()), p).is_ok(),
    }
}

#[test]
fn verdicts_agree_with_and_without_the_calls() {
    let fx = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let transfer_vk = read(&fx.join("transfer/vk"));
    let r16_vk = read(&fx.join("r16/vk"));
    cases(Uh::V6Plain, "2^16", &fx.join("r16"), &transfer_vk, None);
    cases(Uh::V5Zk, "transfer (bb 5, ZK)", &fx.join("transfer"), &r16_vk, Some(&fx.join("transfer/public_inputs.bad")));
    odd_hosts(Uh::V6Plain, "2^16", &fx.join("r16"));
    odd_hosts(Uh::V5Zk, "transfer (bb 5, ZK)", &fx.join("transfer"));
    // the larger circuits, where a checkout has made them (spikes/epoch-proof/measure.sh)
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spikes/epoch-proof/out");
    for (k, name) in [("r20", "2^20"), ("r24", "2^24")] {
        if out.join(k).join("proof/proof").exists() {
            cases(Uh::V6Plain, name, &out.join(k).join("proof"), &r16_vk, None);
        } else {
            println!("  {k}: not made in this checkout, skipped");
        }
    }
}
