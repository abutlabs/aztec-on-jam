//! Gas spike: verify one UltraHonk proof in refine. Payload byte 0 picks the case:
//! 0 = plain (`bb prove --disable_zk`), 1 = zk, 2 = zk with one proof byte flipped (must
//! fail), 3 = zk with a wrong public input (must fail). Proof, vk and public inputs are
//! embedded from ../circuit/target/{plain,zk}. Output: [1] valid, [0] invalid.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
#[cfg(feature = "bb3")]
use uh3::{verify, ProofType};
#[cfg(all(feature = "bb5", not(feature = "bb3")))]
use uh5::{verify, ProofType};

// BN254 pairings overflow polkavm's default stack (zk-jam-service/spikes/groth16-gas).
polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Spike);
struct Spike;

// artifacts/<bb>/<flavour>/: made by measure.sh with that bb version
#[cfg(feature = "bb3")]
macro_rules! art { ($f:literal) => { include_bytes!(concat!("../../artifacts/bb3/", $f)) }; }
#[cfg(all(feature = "bb5", not(feature = "bb3")))]
macro_rules! art { ($f:literal) => { include_bytes!(concat!("../../artifacts/bb5/", $f)) }; }
const PLAIN_PROOF: &[u8] = art!("plain/proof");
const PLAIN_VK: &[u8] = art!("plain/vk");
const PLAIN_PUBS: &[u8] = art!("plain/public_inputs");
const ZK_PROOF: &[u8] = art!("zk/proof");
const ZK_VK: &[u8] = art!("zk/vk");
const ZK_PUBS: &[u8] = art!("zk/public_inputs");

fn pubs(bytes: &[u8]) -> Vec<[u8; 32]> {
    bytes.chunks_exact(32).map(|c| c.try_into().unwrap()).collect()
}

impl Service for Spike {
    fn refine(
        _core_index: CoreIndex,
        _item_index: usize,
        _service_id: ServiceId,
        payload: WorkPayload,
        _package_hash: WorkPackageHash,
    ) -> WorkOutput {
        let data = payload.take();
        let case = data.first().copied().unwrap_or(0);
        let (proof, vk, pi) = match case {
            0 => (ProofType::Plain(Box::from(PLAIN_PROOF)), PLAIN_VK, pubs(PLAIN_PUBS)),
            2 => {
                let mut bad: Vec<u8> = ZK_PROOF.to_vec();
                bad[100] ^= 1;
                (ProofType::ZK(bad.into_boxed_slice()), ZK_VK, pubs(ZK_PUBS))
            }
            3 => {
                let mut bad = pubs(ZK_PUBS);
                bad[0][31] ^= 1;
                (ProofType::ZK(Box::from(ZK_PROOF)), ZK_VK, bad)
            }
            _ => (ProofType::ZK(Box::from(ZK_PROOF)), ZK_VK, pubs(ZK_PUBS)),
        };
        let ok = verify::<()>(vk, &proof, &pi).is_ok();
        vec![if ok { 1u8 } else { 0u8 }].into()
    }

    fn accumulate(_slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        None
    }
}
