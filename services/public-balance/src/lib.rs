//! public-balance: `account (32 bytes) -> u64`, the public money the demo shields from and
//! unshields to, and an ordered log of everything that happened to it, which anyone can read.
//! Dev-chain rules: no operation is signed. Anyone may mint (op 0), and anyone may move any
//! account's balance (ops 1, 2 and 4): this is the faucet's money, not an authenticated ledger.
//!
//! Work-item payload: `[op: u8][account: 32][amount: u64 LE]`, then
//!   op 0 mint, op 1 burn:   `[nonce: 32]`
//!   op 2 shield:            `[dest: u32 LE][note commitment: 32]`, the commitment serving as the nonce
//!   op 4 transfer:          `[nonce: 32][to: 32]`, `account` paying `to`
//! and op 3 publishes how to pay an account privately: `[3][account: 32][X25519 note-encryption
//! key: 32][spend public key: 32]` sets `key:<account>` to the 64 bytes, so a sender can make a
//! note the account's owner can spend (its owner key) and find (sealed to its encryption key).
//! Refine checks the shape and passes it through; accumulate applies it once: a nonce it has seen
//! (`seen:<nonce>`) is skipped, so a builder may resend a package it believes lost without paying
//! twice.
//!
//! Shield: burn `amount` from `account` and send it as a deferred transfer to the privacy
//! service `dest` with the note commitment in the memo. Unshield: a Transfer item from the
//! privacy service carries the receiver account in its memo; credit it.
//!
//! Storage: `bal:<account>` = u64 LE, `seen:<nonce>` = 1, `key:<account>` = 64 bytes, and the
//! log: `events` = how many (u64 LE), `ev:<n: u64 LE>` = one 77-byte record per applied change,
//! `[kind: u8][slot: u32 LE][account: 32][amount: u64 LE][other: 32]`:
//!   kind 0 mint, 1 burn, 2 shield (other: the note commitment), 3 key published (amount 0),
//!   4 transfer (other: the receiving account), 5 a deferred transfer received (account: the
//!   receiver named in its memo, other: the paying service's id, u32 LE).
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage, transfer};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use service_util::{key, le_u64, take};

declare_service!(PublicBalance);
struct PublicBalance;

const HEAD_LEN: usize = 1 + 32 + 8;
const ITEM_LEN: usize = HEAD_LEN + 32;
const SHIELD_LEN: usize = HEAD_LEN + 4 + 32;
const TRANSFER_LEN: usize = ITEM_LEN + 32;
const OP_MINT: u8 = 0;
const OP_BURN: u8 = 1;
const OP_SHIELD: u8 = 2;
const OP_KEY: u8 = 3;
const OP_TRANSFER: u8 = 4;
const EV_RECEIVED: u8 = 5;
const KEY_LEN: usize = 1 + 32 + 64;
const SHIELD_GAS: UnsignedGas = 100_000;

/// Append one record to the public log.
fn log(slot: Slot, kind: u8, account: &[u8], amount: u64, other: &[u8]) {
    let n = get_storage(b"events").map(|v| le_u64(&v)).unwrap_or(0);
    let mut e = Vec::with_capacity(77);
    e.push(kind);
    e.extend_from_slice(&slot.to_le_bytes());
    e.extend_from_slice(&account[..32]);
    e.extend_from_slice(&amount.to_le_bytes());
    let mut o = [0u8; 32];
    o[..other.len().min(32)].copy_from_slice(&other[..other.len().min(32)]);
    e.extend_from_slice(&o);
    set_storage(&key(b"ev:", &n.to_le_bytes()), &e).ok();
    set_storage(b"events", &(n + 1).to_le_bytes()).ok();
}

fn balance(account: &[u8]) -> u64 {
    get_storage(&key(b"bal:", account)).map(|v| le_u64(&v)).unwrap_or(0)
}

fn set_balance(account: &[u8], value: u64) {
    set_storage(&key(b"bal:", account), &value.to_le_bytes()).ok();
}

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
            Some(&OP_KEY) => data.len() == KEY_LEN,
            Some(&OP_TRANSFER) => data.len() == TRANSFER_LEN,
            _ => false,
        };
        if !ok {
            return vec![0u8].into();
        }
        let mut out = vec![1u8];
        out.extend_from_slice(&data);
        out.into()
    }

    fn accumulate(slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let rec = match item {
                AccumulateItem::WorkItem(r) => r,
                AccumulateItem::Transfer(t) => {
                    // an unshield from the privacy service: credit the receiver in the memo
                    let receiver = &t.memo.0[..32];
                    set_balance(receiver, balance(receiver).saturating_add(t.amount));
                    log(slot, EV_RECEIVED, receiver, t.amount, &t.source.to_le_bytes());
                    continue;
                }
            };
            let out = match rec.result {
                Ok(o) => o.0,
                Err(_) => continue,
            };
            if out.first() != Some(&1u8) {
                continue;
            }
            if out.get(1) == Some(&OP_KEY) && out.len() == 1 + KEY_LEN {
                // publishing a key is idempotent: the latest one stands
                set_storage(&key(b"key:", &out[2..34]), &out[34..98]).ok();
                log(slot, OP_KEY, &out[2..34], 0, &[]);
                continue;
            }
            if out.len() < 1 + ITEM_LEN {
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
            let bal = balance(&account);
            let applied = match op {
                OP_MINT => bal.checked_add(amount).map(|n| (n, [0u8; 32])),
                OP_BURN => bal.checked_sub(amount).map(|n| (n, [0u8; 32])),
                OP_SHIELD if out.len() == 1 + SHIELD_LEN => {
                    let dest = u32::from_le_bytes(take::<4>(&out, 1 + HEAD_LEN).unwrap()) as ServiceId;
                    let commitment: [u8; 32] = take(&out, 1 + HEAD_LEN + 4).unwrap();
                    match bal.checked_sub(amount) {
                        Some(n) => {
                            let mut memo = Memo([0u8; 128]);
                            memo.0[..32].copy_from_slice(&commitment);
                            memo.0[32..35].copy_from_slice(b"jam");
                            // the debit stands only if the transfer went
                            if transfer(dest, amount as Balance, SHIELD_GAS, &memo).is_ok() { Some((n, commitment)) } else { None }
                        }
                        None => None,
                    }
                }
                OP_TRANSFER if out.len() == 1 + TRANSFER_LEN => {
                    let to: [u8; 32] = take(&out, 1 + ITEM_LEN).unwrap();
                    bal.checked_sub(amount).map(|n| (n, to))
                }
                _ => None,
            };
            if let Some((n, other)) = applied {
                set_balance(&account, n);
                if op == OP_TRANSFER {
                    // after the debit is written, so paying oneself leaves the balance as it was
                    set_balance(&other, balance(&other).saturating_add(amount));
                }
                set_storage(&seen, &[1u8]).ok();
                log(slot, op, &account, amount, &other);
            }
        }
        None
    }
}
