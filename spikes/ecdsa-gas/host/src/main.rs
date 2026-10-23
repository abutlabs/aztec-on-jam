//! ecdsa-host <vectors dir>: geth's ecRecover.json, then generated.txt (one line per signature
//! made by Python's `cryptography`: hash r s and the signer's uncompressed public key, hex).
//! For each generated one, the recovery id is the one whose recovered key equals the given key;
//! writes <dir>/inputs.txt, the 128-byte precompile input and the signer's address per line.
use std::{env, fs};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

fn main() {
    let dir = env::args().nth(1).expect("usage: ecdsa-host <vectors dir>");
    let g = fs::read_to_string(format!("{dir}/geth-ecRecover.json")).unwrap();
    let (mut pass, mut fail) = (0, 0);
    for chunk in g.split("\"Input\": \"").skip(1) {
        let input = chunk.split('"').next().unwrap();
        let expected = chunk.split("\"Expected\": \"").nth(1).map(|e| e.split('"').next().unwrap()).unwrap_or("");
        let got = ecrecover::ecrecover(&unhex(input)).map(|a| format!("{}{}", "0".repeat(24), hex(&a))).unwrap_or_default();
        if got == expected { pass += 1 } else { fail += 1; println!("MISMATCH geth: want {expected:?}, got {got:?}") }
    }
    println!("geth ecRecover vectors: {pass} agree, {fail} differ");
    let mut out = String::new();
    let (mut gpass, mut gfail) = (0, 0);
    for line in fs::read_to_string(format!("{dir}/generated.txt")).unwrap().lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (hash, r, s, pubkey) = (unhex(f[0]), unhex(f[1]), unhex(f[2]), unhex(f[3]));
        let key = k256::ecdsa::VerifyingKey::from_sec1_bytes(&pubkey).unwrap();
        let want = ecrecover::address(&key);
        let mut found = None;
        for v in [27u8, 28] {
            let mut input = hash.clone();
            input.extend([0u8; 31]); input.push(v);
            input.extend(&r); input.extend(&s);
            if ecrecover::ecrecover(&input) == Some(want) { found = Some(input) }
        }
        match found {
            Some(input) => { gpass += 1; out += &format!("{} {}\n", hex(&input), hex(&want)); }
            None => { gfail += 1; println!("MISMATCH generated: no v recovers {}", hex(&pubkey)) }
        }
    }
    fs::write(format!("{dir}/inputs.txt"), out).unwrap();
    println!("generated signatures: {gpass} recover their signer, {gfail} do not");
    if fail > 0 || gfail > 0 { std::process::exit(1) }
}
