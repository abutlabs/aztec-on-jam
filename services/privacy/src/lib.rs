//! The privacy service, step 1: refine verifies one UltraHonk proof of the transfer
//! circuit (zk, Keccak transcript) against the embedded verification key and hands its
//! public inputs to accumulate; accumulate checks the old roots against the stored ones,
//! refuses a spent nullifier, then writes the nullifiers, the note hashes and the new roots.
//!
//! Work-item payload: `[pubs_len u16 LE][pubs][proof_len u16 LE][proof][two sealed notes]`.
//! Refine output: `[1][pubs][sealed notes]` when the proof verifies and the sealed notes are
//! the ones it commits to (`logs_hash`), `[0]` otherwise.
//! Public inputs, in circuit order (circuits/transfer): old note root, old nullifier root,
//! new note root, new nullifier root, nullifier 0, nullifier 1, note hash 0, note hash 1,
//! unshield amount (0 = none), unshield receiver (a public-balance account), unshield
//! destination (the public-balance service id), deposit amount (0 = none; else nullifier 0
//! is a shield's commitment and `dep:<it>` must hold this amount). Fields are 32-byte BE.
//!
//! Shield: a deferred transfer from the public-balance service arrives as a Transfer item
//! with `memo = note commitment (32) ++ "jam" ++ zeros`; accumulate records
//! `dep:<commitment> = amount` and adds to `shielded`. Unshield: after a valid proof with a
//! non-zero amount, accumulate sends `transfer(dest, amount, memo = receiver ++ "unshield")`.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use jam_pvm_common::accumulate::{accumulate_items, get_storage, remove_storage, set_storage, transfer};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use blake2::{Blake2b, Digest, digest::consts::U32};
use service_util::{be_u64, key, le_u64};
use ultrahonk_no_std::{verify, ProofType};

// BN254 pairings overflow polkavm's default stack (spikes/ultrahonk-gas).
polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Privacy);
struct Privacy;

/// circuits/transfer's verification key (bb 5.0.0, `write_vk -t evm`: zk, Keccak). A preimage later.
const VK: &[u8] = include_bytes!("../fixtures/transfer.vk");

const HASH_LEN: usize = 32;
const N_PUBS: usize = 14;
const OLD_NOTE_ROOT: usize = 0;
const OLD_NULL_ROOT: usize = 1;
const LOGS_HASH: usize = 2;
const OLD_NULL_SIZE: usize = 3;
const NEW_NOTE_ROOT: usize = 4;
const NEW_NULL_ROOT: usize = 5;
const NULLIFIERS: core::ops::Range<usize> = 6..8;
const NOTE_HASHES: core::ops::Range<usize> = 8..10;
const UNSHIELD_AMOUNT: usize = 10;
const UNSHIELD_RECEIVER: usize = 11;
const UNSHIELD_DEST: usize = 12;
const DEPOSIT_AMOUNT: usize = 13;

/// The trees a new chain starts from, so the first proof cannot start from a tree of its own
/// making: the empty depth-40 note tree, and the depth-40 nullifier tree holding only its zero leaf
/// (H(0, 0, 0) at index 0; size 1). client/poseidon2.py and lib/core.js compute the same.
const EMPTY_NOTE_ROOT: [u8; 32] = hex32("1fd848aa69e1633722fe249a5b7f53b094f1c9cef9f5c694b073fd1cc5850dfb");
const EMPTY_NULL_ROOT: [u8; 32] = hex32("22ce2ee466581b1bd5ddd24c066854b58521fd41637f92dfd04c092d21bca4b6");
const EMPTY_NULL_SIZE: u64 = 1;
const KEY_NULL_SIZE: &[u8] = b"null:size";

const fn hex32(s: &str) -> [u8; 32] {
    let b = s.as_bytes();
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        let hi = (b[2 * i] as char).to_digit(16).unwrap() as u8;
        let lo = (b[2 * i + 1] as char).to_digit(16).unwrap() as u8;
        out[i] = hi << 4 | lo;
        i += 1;
    }
    out
}
/// A sealed note (client/notecrypt.py): ephemeral X25519 key, AES-256-GCM ciphertext, tag.
const SEALED_LEN: usize = 32 + 40 + 16;
const LOGS_LEN: usize = 2 * SEALED_LEN;
const UNSHIELD_GAS: UnsignedGas = 100_000;

const KEY_NOTE_ROOT: &[u8] = b"root:note";
const KEY_NULL_ROOT: &[u8] = b"root:null";
const KEY_TRANSFERS: &[u8] = b"transfers";
const KEY_LEAVES: &[u8] = b"leaves";
const KEY_SHIELDED: &[u8] = b"shielded";
const KEY_UNSHIELDED: &[u8] = b"unshielded";

fn invalid() -> WorkOutput {
    vec![0u8].into()
}

/// A u16-LE length, then that many bytes: (them, the rest).
fn take_sized(b: &[u8]) -> Option<(&[u8], &[u8])> {
    let n = u16::from_le_bytes([*b.first()?, *b.get(1)?]) as usize;
    let rest = b.get(2..)?;
    (rest.len() >= n).then(|| rest.split_at(n))
}

/// blake2b-256 of the sealed notes with the first byte zeroed: a BN254 field element.
fn logs_hash(logs: &[u8]) -> [u8; HASH_LEN] {
    let mut h = Blake2b::<U32>::new();
    h.update(logs);
    let mut out: [u8; HASH_LEN] = h.finalize().into();
    out[0] = 0;
    out
}



impl Service for Privacy {
    fn refine(
        _core_index: CoreIndex,
        _item_index: usize,
        _service_id: ServiceId,
        payload: WorkPayload,
        _package_hash: WorkPackageHash,
    ) -> WorkOutput {
        let data = payload.take();
        // [pubs_len u16][pubs][proof_len u16][proof][logs: the two sealed notes]
        let Some((pubs_bytes, rest)) = take_sized(&data) else { return invalid() };
        let Some((proof, logs)) = take_sized(rest) else { return invalid() };
        if pubs_bytes.len() != N_PUBS * HASH_LEN || logs.len() != LOGS_LEN {
            return invalid();
        }
        let pubs: Vec<[u8; HASH_LEN]> = pubs_bytes
            .chunks_exact(HASH_LEN)
            .map(|c| c.try_into().unwrap())
            .collect();
        // the sealed notes are the ones the prover committed to
        if logs_hash(logs) != pubs[LOGS_HASH] {
            return invalid();
        }
        if verify::<()>(VK, &ProofType::ZK(Box::from(proof)), &pubs).is_err() {
            return invalid();
        }
        let mut out = Vec::with_capacity(1 + pubs_bytes.len() + LOGS_LEN);
        out.push(1u8);
        out.extend_from_slice(pubs_bytes);
        out.extend_from_slice(logs);
        out.into()
    }

    fn accumulate(_slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let rec = match item {
                AccumulateItem::WorkItem(r) => r,
                AccumulateItem::Transfer(t) => {
                    // a shield: the public-balance service sent JAM balance with the note
                    // commitment in the memo; the note itself is proven into the tree later
                    let commitment = &t.memo.0[..HASH_LEN];
                    set_storage(&key(b"dep:", commitment), &t.amount.to_le_bytes()).ok();
                    let total = get_storage(KEY_SHIELDED).map(|v| le_u64(&v)).unwrap_or(0);
                    set_storage(KEY_SHIELDED, &(total + t.amount).to_le_bytes()).ok();
                    continue;
                }
            };
            let out = match rec.result {
                Ok(o) => o.0,
                Err(_) => continue,
            };
            if out.len() != 1 + N_PUBS * HASH_LEN + LOGS_LEN || out[0] != 1u8 {
                continue;
            }
            let pubs: Vec<&[u8]> = out[1..1 + N_PUBS * HASH_LEN].chunks_exact(HASH_LEN).collect();
            let logs = &out[1 + N_PUBS * HASH_LEN..];

            // The proof must start from the trees the chain holds: the empty trees on a new chain.
            let stored_note = get_storage(KEY_NOTE_ROOT).unwrap_or_else(|| EMPTY_NOTE_ROOT.to_vec());
            let stored_null = get_storage(KEY_NULL_ROOT).unwrap_or_else(|| EMPTY_NULL_ROOT.to_vec());
            let null_size = get_storage(KEY_NULL_SIZE).map(|v| le_u64(&v)).unwrap_or(EMPTY_NULL_SIZE);
            if stored_note.as_slice() != pubs[OLD_NOTE_ROOT]
                || stored_null.as_slice() != pubs[OLD_NULL_ROOT]
                || be_u64(pubs[OLD_NULL_SIZE]) != null_size
            {
                continue; // stale: another package moved the trees first
            }
            if pubs[NULLIFIERS].iter().any(|nf| get_storage(&key(b"nf:", nf)).is_some()) {
                continue; // a spent note: double spend
            }
            // a deposit claim: nullifier 0 is the shield's commitment, and the shield must
            // have brought exactly the amount the proof says
            let deposit = be_u64(pubs[DEPOSIT_AMOUNT]);
            if deposit > 0 {
                let dep_key = key(b"dep:", pubs[NULLIFIERS.start]);
                match get_storage(&dep_key) {
                    Some(v) if le_u64(&v) == deposit => { remove_storage(&dep_key); }
                    _ => continue, // no such deposit, or not that amount
                }
            }
            // the proof showed both absent from the nullifier tree and inserted them; record them,
            // in order, so any client can rebuild the tree (nfl:<index>), and by value (nf:)
            for (i, nf) in pubs[NULLIFIERS].iter().enumerate() {
                set_storage(&key(b"nf:", nf), &[1u8]).ok();
                set_storage(&key(b"nfl:", &(null_size + i as u64).to_le_bytes()), nf).ok();
            }
            set_storage(KEY_NULL_SIZE, &(null_size + 2).to_le_bytes()).ok();
            // the tree's leaves in order, so a client can rebuild the tree from the chain
            let mut leaves = get_storage(KEY_LEAVES).map(|v| le_u64(&v)).unwrap_or(0);
            for (i, nh) in pubs[NOTE_HASHES].iter().enumerate() {
                set_storage(&key(b"nh:", nh), &[1u8]).ok();
                set_storage(&key(b"leaf:", &leaves.to_le_bytes()), nh).ok();
                // the note's opening, sealed to its owner: a receiver scans these
                set_storage(&key(b"enc:", &leaves.to_le_bytes()), &logs[i * SEALED_LEN..(i + 1) * SEALED_LEN]).ok();
                leaves += 1;
            }
            set_storage(KEY_LEAVES, &leaves.to_le_bytes()).ok();
            set_storage(KEY_NOTE_ROOT, pubs[NEW_NOTE_ROOT]).ok();
            set_storage(KEY_NULL_ROOT, pubs[NEW_NULL_ROOT]).ok();
            let n = get_storage(KEY_TRANSFERS).map(|v| le_u64(&v)).unwrap_or(0);
            set_storage(KEY_TRANSFERS, &(n + 1).to_le_bytes()).ok();

            // unshield: the proof names an amount, a public receiver and the public service
            let amount = be_u64(pubs[UNSHIELD_AMOUNT]);
            if amount > 0 {
                let dest = be_u64(pubs[UNSHIELD_DEST]) as ServiceId;
                let mut memo = Memo([0u8; 128]);
                memo.0[..HASH_LEN].copy_from_slice(pubs[UNSHIELD_RECEIVER]);
                memo.0[HASH_LEN..HASH_LEN + 8].copy_from_slice(b"unshield");
                if transfer(dest, amount as Balance, UNSHIELD_GAS, &memo).is_ok() {
                    let u = get_storage(KEY_UNSHIELDED).map(|v| le_u64(&v)).unwrap_or(0);
                    set_storage(KEY_UNSHIELDED, &(u + amount).to_le_bytes()).ok();
                }
            }
        }
        None
    }
}
