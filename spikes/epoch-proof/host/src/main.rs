//! epoch-proof-host <dir> [zk]: verify <dir>/{proof,vk,public_inputs} (bb's binary output) as a
//! plain (or zk) UltraHonk proof with the verifier refine runs. Prints the verdict and the error.
use std::{env, fs};
use uh6::{verify, ProofType};

fn main() {
    let d = env::args().nth(1).expect("usage: epoch-proof-host <dir>");
    let read = |f: &str| fs::read(format!("{d}/{f}")).unwrap_or_else(|e| panic!("{d}/{f}: {e}"));
    let (proof, vk, pubs) = (read("proof"), read("vk"), read("public_inputs"));
    let pi: Vec<[u8; 32]> = pubs.chunks_exact(32).map(|c| c.try_into().unwrap()).collect();
    println!("proof {} bytes, vk {} bytes, {} public inputs", proof.len(), vk.len(), pi.len());
    let zk = env::args().nth(2).as_deref() == Some("zk");
    let proof = if zk { ProofType::ZK(proof.into_boxed_slice()) } else { ProofType::Plain(proof.into_boxed_slice()) };
    match verify::<()>(&vk, &proof, &pi) {
        Ok(()) => println!("accepted"),
        Err(e) => println!("rejected: {e:?}"),
    }
}
