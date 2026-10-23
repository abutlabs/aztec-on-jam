//! public-balance: `account (32 bytes) -> u64`, the public money the demo shields from and
//! unshields to. Dev-chain rules: anyone may mint (op 0); a burn (op 1) is what a shield will
//! become once the deferred transfer to the privacy service is wired (step 1, part three).
//!
//! Work-item payload: `[op: u8][account: 32][amount: u64 LE]`, then for a mint or burn a
//! 32-byte nonce, and for a shield (op 2) `[dest: u32 LE][note commitment: 32]`, the
//! commitment serving as the nonce. Refine checks the shape and passes it through;
//! accumulate applies it once: a nonce it has seen (`seen:<nonce>`) is skipped, so a builder
//! may resend a package it believes lost (lasair#93) without paying twice.
//! Storage: `bal:<account>` = u64 LE, `seen:<nonce>` = 1.
//!
//! Shield: burn `amount` from `account` and send it as a deferred transfer to the privacy
//! service `dest` with the note commitment in the memo. Unshield: a Transfer item from the
//! privacy service carries the receiver account in its memo; credit it.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;

use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage, transfer};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use service_util::{key, le_u64, take};

declare_service!(PublicBalance);
struct PublicBalance;

const HEAD_LEN: usize = 1 + 32 + 8;
const ITEM_LEN: usize = HEAD_LEN + 32;
const SHIELD_LEN: usize = HEAD_LEN + 4 + 32;
const OP_MINT: u8 = 0;
const OP_BURN: u8 = 1;
const OP_SHIELD: u8 = 2;
const SHIELD_GAS: UnsignedGas = 100_000;

impl Service for PublicBalance {
    fn refine(
        _core_index: CoreIndex,
        _item_index: usize,
        _service_id: ServiceId,
        payload: WorkPayload,
        _package_hash: WorkPackageHash,
    ) -> WorkOutput {
        let data = payload.take();
        let ok = match data.first() {
            Some(&OP_MINT) | Some(&OP_BURN) => data.len() == ITEM_LEN,
            Some(&OP_SHIELD) => data.len() == SHIELD_LEN,
            _ => false,
        };
        if !ok {
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
                AccumulateItem::Transfer(t) => {
                    // an unshield from the privacy service: credit the receiver in the memo
                    let receiver = &t.memo.0[..32];
                    let k = key(b"bal:", receiver);
                    let bal = get_storage(&k).map(|v| le_u64(&v)).unwrap_or(0);
                    set_storage(&k, &bal.saturating_add(t.amount as u64).to_le_bytes()).ok();
                    continue;
                }
            };
            let out = match rec.result {
                Ok(o) => o.0,
                Err(_) => continue,
            };
            if out.len() < 1 + ITEM_LEN || out[0] != 1u8 {
                continue;
            }
            let op = out[1];
            // the nonce: 32 bytes after the head, a mint's own or a shield's commitment
            let nonce_at = if op == OP_SHIELD { 1 + HEAD_LEN + 4 } else { 1 + HEAD_LEN };
            let seen = match out.get(nonce_at..nonce_at + 32) { Some(n) => key(b"seen:", n), None => continue };
            if get_storage(&seen).is_some() {
                continue; // applied already: a resent package
            }
            let account: [u8; 32] = match take(&out, 2) { Some(a) => a, None => continue };
            let amount = le_u64(&out[34..42]);
            let k = key(b"bal:", &account);
            let bal = get_storage(&k).map(|v| le_u64(&v)).unwrap_or(0);
            let new = match op {
                OP_MINT => bal.checked_add(amount),
                OP_BURN => bal.checked_sub(amount),
                OP_SHIELD if out.len() == 1 + SHIELD_LEN => {
                    let dest = u32::from_le_bytes(take::<4>(&out, 1 + HEAD_LEN).unwrap()) as ServiceId;
                    let commitment: [u8; 32] = take(&out, 1 + HEAD_LEN + 4).unwrap();
                    match bal.checked_sub(amount) {
                        Some(n) => {
                            let mut memo = Memo([0u8; 128]);
                            memo.0[..32].copy_from_slice(&commitment);
                            memo.0[32..35].copy_from_slice(b"jam");
                            // the debit stands only if the transfer went
                            if transfer(dest, amount as Balance, SHIELD_GAS, &memo).is_ok() { Some(n) } else { None }
                        }
                        None => None,
                    }
                }
                _ => None,
            };
            if let Some(n) = new {
                set_storage(&k, &n.to_le_bytes()).ok();
                set_storage(&seen, &[1u8]).ok();
            }
        }
        None
    }
}
