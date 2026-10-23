//! The privacy service, step 1: refine verifies one UltraHonk proof of the transfer
//! circuit (zk, Keccak transcript) against the embedded verification key and hands its
//! public inputs to accumulate; accumulate checks the old roots against the stored ones,
//! refuses a spent nullifier, then writes the nullifiers, the note hashes and the new roots.
//!
//! Work-item payload: `[pubs_len: u16 LE][pubs: pubs_len bytes, 32 each][proof: the rest]`.
//! Refine output: `[1][pubs]` when the proof verifies, `[0]` otherwise.
//! Public inputs, in circuit order (circuits/transfer_stub): old note root, old nullifier
//! root, new note root, new nullifier root, nullifier 0, nullifier 1, note hash 0, note hash 1.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use service_util::{key, le_u64};
use ultrahonk_no_std::{verify, ProofType};

// BN254 pairings overflow polkavm's default stack (spikes/ultrahonk-gas).
polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Privacy);
struct Privacy;

/// The transfer circuit's verification key (bb 3.0.3, `--oracle_hash keccak`). A preimage later.
const VK: &[u8] = include_bytes!("../fixtures/transfer_stub.vk");

const HASH_LEN: usize = 32;
const N_PUBS: usize = 8;
const OLD_NOTE_ROOT: usize = 0;
const OLD_NULL_ROOT: usize = 1;
const NEW_NOTE_ROOT: usize = 2;
const NEW_NULL_ROOT: usize = 3;
const NULLIFIERS: core::ops::Range<usize> = 4..6;
const NOTE_HASHES: core::ops::Range<usize> = 6..8;

const KEY_NOTE_ROOT: &[u8] = b"root:note";
const KEY_NULL_ROOT: &[u8] = b"root:null";
const KEY_TRANSFERS: &[u8] = b"transfers";

fn invalid() -> WorkOutput {
    vec![0u8].into()
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
        if data.len() < 2 {
            return invalid();
        }
        let pubs_len = u16::from_le_bytes([data[0], data[1]]) as usize;
        if pubs_len % HASH_LEN != 0 || data.len() < 2 + pubs_len {
            return invalid();
        }
        let pubs_bytes = &data[2..2 + pubs_len];
        let proof = &data[2 + pubs_len..];
        let pubs: Vec<[u8; HASH_LEN]> = pubs_bytes
            .chunks_exact(HASH_LEN)
            .map(|c| c.try_into().unwrap())
            .collect();
        let proof = ProofType::ZK(Box::from(proof));
        if verify::<()>(VK, &proof, &pubs).is_err() {
            return invalid();
        }
        let mut out = Vec::with_capacity(1 + pubs_len);
        out.push(1u8);
        out.extend_from_slice(pubs_bytes);
        out.into()
    }

    fn accumulate(_slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let rec = match item {
                AccumulateItem::WorkItem(r) => r,
                _ => continue, // deferred transfers (shield) arrive in a later step
            };
            let out = match rec.result {
                Ok(o) => o.0,
                Err(_) => continue,
            };
            if out.len() != 1 + N_PUBS * HASH_LEN || out[0] != 1u8 {
                continue;
            }
            let pubs: Vec<&[u8]> = out[1..].chunks_exact(HASH_LEN).collect();

            // The proof must start from the roots the chain holds (an empty chain accepts any).
            let stored_note = get_storage(KEY_NOTE_ROOT).unwrap_or_default();
            let stored_null = get_storage(KEY_NULL_ROOT).unwrap_or_default();
            if (!stored_note.is_empty() && stored_note.as_slice() != pubs[OLD_NOTE_ROOT])
                || (!stored_null.is_empty() && stored_null.as_slice() != pubs[OLD_NULL_ROOT])
            {
                continue; // stale root: another package moved the tree first
            }
            if pubs[NULLIFIERS].iter().any(|nf| get_storage(&key(b"nf:", nf)).is_some()) {
                continue; // a spent note: double spend
            }
            for nf in &pubs[NULLIFIERS] {
                set_storage(&key(b"nf:", nf), &[1u8]).ok();
            }
            for nh in &pubs[NOTE_HASHES] {
                set_storage(&key(b"nh:", nh), &[1u8]).ok(); // the encrypted note goes here later
            }
            set_storage(KEY_NOTE_ROOT, pubs[NEW_NOTE_ROOT]).ok();
            set_storage(KEY_NULL_ROOT, pubs[NEW_NULL_ROOT]).ok();
            let n = get_storage(KEY_TRANSFERS).map(|v| le_u64(&v)).unwrap_or(0);
            set_storage(KEY_TRANSFERS, &(n + 1).to_le_bytes()).ok();
        }
        None
    }
}
