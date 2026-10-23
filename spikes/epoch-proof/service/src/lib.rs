//! Spike: verify one bb 6.0.0 UltraHonk (Keccak) proof in refine, as JAM would check Aztec's
//! epoch proof. The payload carries everything, so one blob meters any circuit:
//!   [flavour u8: 0 plain, 1 zk][tag u8][vk_len u16 LE][vk][n_pubs u16 LE][n_pubs × 32][proof]
//! Refine outputs [tag][1 valid | 0 invalid]; accumulate stores it at "v:" ++ tag, for
//! `jamboree rpc read`. PolkaJam's telemetry meters refine's gas and time.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;

use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use ultrahonk_no_std::{verify, ProofType};

// BN254 pairings overflow polkavm's default stack (zk-jam-service/spikes/groth16-gas).
polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Spike);
struct Spike;

fn le16(b: &[u8]) -> Option<usize> {
    Some(u16::from_le_bytes(b.get(..2)?.try_into().ok()?) as usize)
}

/// (tag, verdict); a payload that doesn't parse is invalid.
fn check(p: &[u8]) -> (u8, bool) {
    let tag = p.get(1).copied().unwrap_or(0);
    let parsed = (|| {
        let zk = *p.first()? == 1;
        let vk_len = le16(&p[2..])?;
        let vk = p.get(4..4 + vk_len)?;
        let at = 4 + vk_len;
        let n = le16(&p[at..])?;
        let pubs: Vec<[u8; 32]> = p.get(at + 2..at + 2 + 32 * n)?.chunks_exact(32).map(|c| c.try_into().unwrap()).collect();
        let proof = p.get(at + 2 + 32 * n..)?;
        let proof = if zk { ProofType::ZK(proof.into()) } else { ProofType::Plain(proof.into()) };
        Some(verify::<()>(vk, &proof, &pubs).is_ok())
    })();
    (tag, parsed.unwrap_or(false))
}

impl Service for Spike {
    fn refine(
        _core_index: CoreIndex,
        _item_index: usize,
        _service_id: ServiceId,
        payload: WorkPayload,
        _package_hash: WorkPackageHash,
    ) -> WorkOutput {
        let (tag, ok) = check(&payload.take());
        alloc::vec![tag, ok as u8].into()
    }

    fn accumulate(_slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        for item in accumulate_items() {
            if let AccumulateItem::WorkItem(rec) = item {
                if let Ok(out) = rec.result {
                    if out.0.len() == 2 {
                        set_storage(&[b'v', b':', out.0[0]], &[out.0[1]]).ok();
                    }
                }
            }
        }
        None
    }
}
