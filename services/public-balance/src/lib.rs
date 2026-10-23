//! public-balance: `account (32 bytes) -> u64`, the public money the demo shields from and
//! unshields to. Dev-chain rules: anyone may mint (op 0); a burn (op 1) is what a shield will
//! become once the deferred transfer to the privacy service is wired (step 1, part three).
//!
//! Work-item payload: `[op: u8][account: 32][amount: u64 LE]`. Refine checks the shape and
//! passes it through; accumulate applies it. Storage: `bal:<account>` = u64 LE.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;

use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use service_util::{key, le_u64, take};

declare_service!(PublicBalance);
struct PublicBalance;

const ITEM_LEN: usize = 1 + 32 + 8;
const OP_MINT: u8 = 0;
const OP_BURN: u8 = 1;

impl Service for PublicBalance {
    fn refine(
        _core_index: CoreIndex,
        _item_index: usize,
        _service_id: ServiceId,
        payload: WorkPayload,
        _package_hash: WorkPackageHash,
    ) -> WorkOutput {
        let data = payload.take();
        if data.len() != ITEM_LEN || data[0] > OP_BURN {
            return vec![0u8].into();
        }
        let mut out = vec![1u8];
        out.extend_from_slice(&data);
        out.into()
    }

    fn accumulate(_slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let rec = match item {
                AccumulateItem::WorkItem(r) => r,
                _ => continue,
            };
            let out = match rec.result {
                Ok(o) => o.0,
                Err(_) => continue,
            };
            if out.len() != 1 + ITEM_LEN || out[0] != 1u8 {
                continue;
            }
            let op = out[1];
            let account: [u8; 32] = match take(&out, 2) { Some(a) => a, None => continue };
            let amount = le_u64(&out[34..42]);
            let k = key(b"bal:", &account);
            let bal = get_storage(&k).map(|v| le_u64(&v)).unwrap_or(0);
            let new = match op {
                OP_MINT => bal.checked_add(amount),
                OP_BURN => bal.checked_sub(amount),
                _ => None,
            };
            if let Some(n) = new {
                set_storage(&k, &n.to_le_bytes()).ok();
            }
        }
        None
    }
}
