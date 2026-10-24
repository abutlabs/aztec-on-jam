//! A JAM service standing in for an L1 contract that sends Aztec messages (#50): payload
//! [destination u32 LE][k u8][k × 128-byte memo]; accumulate sends each memo to the destination
//! as a deferred transfer of 1 unit, the settlement service's Inbox reading it as a message from
//! this service. Records how many went at "sent".
#![no_std]
#![no_main]

extern crate alloc;

use jam_pvm_common::accumulate::{accumulate_items, set_storage, transfer};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};

declare_service!(Sender);
struct Sender;

/// Gas the destination's accumulate gets for each message: two sha256s and three storage writes.
const MESSAGE_GAS: u64 = 2_000_000;

impl Service for Sender {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        payload.take().into()
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let AccumulateItem::WorkItem(rec) = item else { continue };
            let Ok(out) = rec.result else { continue };
            let p = out.0;
            let (Some(dest), Some(&k)) = (p.get(..4), p.get(4)) else { continue };
            let dest = u32::from_le_bytes(dest.try_into().unwrap());
            let mut sent = 0u32;
            for m in p[5..].chunks_exact(128).take(k as usize) {
                let mut memo = Memo([0u8; 128]);
                memo.0.copy_from_slice(m);
                if transfer(dest, 1, MESSAGE_GAS, &memo).is_ok() {
                    sent += 1;
                }
            }
            set_storage(b"sent", &sent.to_le_bytes()).ok();
        }
        None
    }
}
