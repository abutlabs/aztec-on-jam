//! Spike: Ethereum's ecrecover in refine, as Aztec's settlement checks a proposer's signature and
//! a committee's attestations (#39). Payload: [tag][n][n × (precompile input 128 || signer 20)].
//! Refine outputs [tag][1 every signature recovers its signer | 0]; accumulate stores it at
//! "v:" ++ tag.
#![no_std]
#![no_main]

extern crate alloc;

use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(1024 * 1024);

declare_service!(Spike);
struct Spike;

fn all_recover(d: &[u8]) -> Option<bool> {
    let n = *d.first()? as usize;
    let items = d.get(1..)?;
    if n == 0 || items.len() != n * 148 {
        return None;
    }
    Some(items.chunks_exact(148).all(|it| ecrecover::ecrecover(&it[..128]).as_ref().map(|a| &a[..]) == Some(&it[128..])))
}

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.first().copied().unwrap_or(0);
        let ok = p.get(1..).and_then(all_recover).unwrap_or(false);
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
