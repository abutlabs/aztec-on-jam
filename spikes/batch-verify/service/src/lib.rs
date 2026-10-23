//! #30: one refine verifies n plain UltraHonk proofs. Payload: [tag][n] then n items of
//! [vk_len u16][vk][pubs_n u16][pubs][proof_len u32][proof]. Output [tag][1 all valid | 0];
//! accumulate stores it at "v:" ++ tag.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use ultrahonk_no_std::{verify, ProofType};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Batch);
struct Batch;

fn all_valid(p: &[u8]) -> Option<bool> {
    let n = *p.first()? as usize;
    let mut at = 1;
    for _ in 0..n {
        let vk_len = u16::from_le_bytes(p.get(at..at + 2)?.try_into().ok()?) as usize;
        let vk = p.get(at + 2..at + 2 + vk_len)?;
        at += 2 + vk_len;
        let pn = u16::from_le_bytes(p.get(at..at + 2)?.try_into().ok()?) as usize;
        let pubs: Vec<[u8; 32]> = p.get(at + 2..at + 2 + 32 * pn)?.chunks_exact(32).map(|c| c.try_into().unwrap()).collect();
        at += 2 + 32 * pn;
        let len = u32::from_le_bytes(p.get(at..at + 4)?.try_into().ok()?) as usize;
        let proof = p.get(at + 4..at + 4 + len)?;
        at += 4 + len;
        if verify::<()>(vk, &ProofType::Plain(proof.into()), &pubs).is_err() {
            return Some(false);
        }
    }
    Some(at == p.len())
}

impl Service for Batch {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.first().copied().unwrap_or(0);
        let ok = p.get(1..).and_then(all_valid).unwrap_or(false);
        alloc::vec![tag, ok as u8].into()
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
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
