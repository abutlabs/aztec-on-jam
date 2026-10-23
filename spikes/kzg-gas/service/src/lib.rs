//! Spike: one EIP-4844 point evaluation in refine, the check Ethereum's 0x0a precompile does
//! for Aztec's epoch proof (BlobLib.sol). Payload: [tag][192-byte precompile input]. Refine
//! outputs [tag][1 valid | 0 invalid or refused]; accumulate stores it at "v:" ++ tag.
#![no_std]
#![no_main]

extern crate alloc;

use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Spike);
struct Spike;

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.first().copied().unwrap_or(0);
        let ok = p.get(1..).map(|input| kzg_verify::point_evaluation(input) == Ok(true)).unwrap_or(false);
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
