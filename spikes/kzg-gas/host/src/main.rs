//! kzg-host <vectors dir>: every c-kzg-4844 verify_kzg_proof vector (ckzg/*.yaml: output true,
//! false, or null for an input that must be refused) and geth's pointEvaluation.json vector.
//! KZG_CALLS=absent plays a net without the curve host calls 103/104 (kzg-verify's PVM-code
//! path); by default the calls answer, through curve-calls' native stand-in.
use std::{env, fs};

fn hex<const N: usize>(s: &str) -> Option<[u8; N]> {
    let s = s.trim().trim_matches('\'').trim_start_matches("0x");
    if s.len() != 2 * N { return None; }
    let mut out = [0u8; N];
    for i in 0..N { out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?; }
    Some(out)
}

fn field<'a>(y: &'a str, k: &str) -> &'a str {
    y.lines().find_map(|l| l.trim().strip_prefix(&format!("{k}: "))).unwrap_or("")
}

fn main() {
    let dir = env::args().nth(1).expect("usage: kzg-host <vectors dir>");
    // each vector from a fresh state, as each refine starts: the probe, then the path it picks
    let absent = env::var("KZG_CALLS").as_deref() == Ok("absent");
    let fresh = || curve_calls::stand_in::set_absent(absent);
    let verify_kzg_proof = |c: &[u8; 48], z: &[u8; 32], v: &[u8; 32], pi: &[u8; 48]| { fresh(); kzg_verify::verify_kzg_proof(c, z, v, pi) };
    let verify_blob_kzg_proof = |b: &[u8], c: &[u8; 48], pi: &[u8; 48]| { fresh(); kzg_verify::verify_blob_kzg_proof(b, c, pi) };
    let verify_blob_kzg_proof_batch = |b: &[&[u8]], c: &[[u8; 48]], pi: &[[u8; 48]]| { fresh(); kzg_verify::verify_blob_kzg_proof_batch(b, c, pi) };
    let point_evaluation = |i: &[u8]| { fresh(); kzg_verify::point_evaluation(i) };
    let (mut pass, mut fail) = (0, 0);
    let mut names: Vec<_> = fs::read_dir(format!("{dir}/ckzg")).unwrap().map(|e| e.unwrap().path()).collect();
    names.sort();
    for p in names {
        let y = fs::read_to_string(&p).unwrap();
        let want = field(&y, "output");
        // a field of the wrong length counts as refused input, as c-kzg's bindings do
        let got = match (hex::<48>(field(&y, "commitment")), hex::<32>(field(&y, "z")), hex::<32>(field(&y, "y")), hex::<48>(field(&y, "proof"))) {
            (Some(c), Some(z), Some(v), Some(pi)) => match verify_kzg_proof(&c, &z, &v, &pi) {
                Ok(true) => "true", Ok(false) => "false", Err(_) => "null",
            },
            _ => "null",
        };
        if got == want { pass += 1 } else { fail += 1; println!("MISMATCH {}: want {want}, got {got}", p.display()) }
    }
    // c-kzg-4844's verify_blob_kzg_proof vectors (blob/*.yaml)
    let (mut bpass, mut bfail) = (0, 0);
    let mut blobs: Vec<_> = fs::read_dir(format!("{dir}/blob")).unwrap().map(|e| e.unwrap().path()).collect();
    blobs.sort();
    for p in blobs {
        let y = fs::read_to_string(&p).unwrap();
        let want = field(&y, "output");
        let blob_hex = field(&y, "blob").trim_matches('\'').trim_start_matches("0x");
        let blob: Option<Vec<u8>> = (blob_hex.len() % 2 == 0).then(|| (0..blob_hex.len() / 2).map(|i| u8::from_str_radix(&blob_hex[2 * i..2 * i + 2], 16).ok()).collect::<Option<Vec<u8>>>()).flatten();
        let got = match (blob, hex::<48>(field(&y, "commitment")), hex::<48>(field(&y, "proof"))) {
            (Some(b), Some(c), Some(pi)) => match verify_blob_kzg_proof(&b, &c, &pi) {
                Ok(true) => "true", Ok(false) => "false", Err(_) => "null",
            },
            _ => "null",
        };
        if got == want { bpass += 1 } else { bfail += 1; println!("MISMATCH {}: want {want}, got {got}", p.display()) }
    }
    println!("c-kzg verify_blob_kzg_proof vectors: {bpass} agree, {bfail} differ");
    // c-kzg-4844's verify_blob_kzg_proof_batch vectors (batch/*.yaml): lists of blobs,
    // commitments and proofs, one per line under each key
    let (mut tpass, mut tfail) = (0, 0);
    let mut batches: Vec<_> = fs::read_dir(format!("{dir}/batch")).unwrap().map(|e| e.unwrap().path()).collect();
    batches.sort();
    for p in batches {
        let y = fs::read_to_string(&p).unwrap();
        let want = field(&y, "output");
        let list = |k: &str| -> Vec<String> {
            let mut on = false; let mut out = vec![];
            for l in y.lines() {
                let t = l.trim();
                if t == format!("{k}:") || t == format!("{k}: []") { on = t.ends_with(':'); continue; }
                if on { if let Some(v) = t.strip_prefix("- ") { out.push(v.trim_matches('\'').trim_start_matches("0x").to_string()) } else if !t.is_empty() { on = false } }
            }
            out
        };
        let unhex = |h: &str| -> Option<Vec<u8>> { (h.len() % 2 == 0).then(|| (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()).collect::<Option<Vec<u8>>>()).flatten() };
        let blobs: Option<Vec<Vec<u8>>> = list("blobs").iter().map(|h| unhex(h)).collect();
        let cs: Option<Vec<[u8; 48]>> = list("commitments").iter().map(|h| hex::<48>(h)).collect();
        let pis: Option<Vec<[u8; 48]>> = list("proofs").iter().map(|h| hex::<48>(h)).collect();
        let got = match (blobs, cs, pis) {
            (Some(b), Some(c), Some(pi)) => {
                let bs: Vec<&[u8]> = b.iter().map(|x| x.as_slice()).collect();
                match verify_blob_kzg_proof_batch(&bs, &c, &pi) { Ok(true) => "true", Ok(false) => "false", Err(_) => "null" }
            }
            _ => "null",
        };
        if got == want { tpass += 1 } else { tfail += 1; println!("MISMATCH {}: want {want}, got {got}", p.display()) }
    }
    println!("c-kzg verify_blob_kzg_proof_batch vectors: {tpass} agree, {tfail} differ");
    let g = fs::read_to_string(format!("{dir}/geth-pointEvaluation.json")).unwrap();
    let input = g.split("\"Input\": \"").nth(1).unwrap().split('"').next().unwrap();
    let bytes: Vec<u8> = (0..input.len() / 2).map(|i| u8::from_str_radix(&input[2 * i..2 * i + 2], 16).unwrap()).collect();
    let geth = point_evaluation(&bytes);
    let mut bad = bytes.clone(); bad[70] ^= 1; // y altered
    let geth_bad = point_evaluation(&bad);
    println!("c-kzg verify_kzg_proof vectors: {pass} agree, {fail} differ");
    println!("geth pointEvaluation1: {geth:?}; with y altered: {geth_bad:?}");
    if fail > 0 || bfail > 0 || tfail > 0 || geth != Ok(true) || geth_bad == Ok(true) { std::process::exit(1) }
    println!("curve calls 101-104 used (Some(true) the call, Some(false) PVM code): {:?}", curve_calls::used());
}
