//! The privacy service. Scaffold: the entry points and the item layout, no verifier yet.
//! Step 0 of docs/PLAN.md (the verifier measurement) decides what refine verifies.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};

declare_service!(Privacy);
struct Privacy;

/// Storage keys. Roots are 32 bytes; a nullifier key is its 32 bytes behind `nf:`.
const KEY_NOTE_ROOT: &[u8] = b"root:note";
const KEY_NULL_ROOT: &[u8] = b"root:null";
const HASH_LEN: usize = 32;

/// Refine's output, read by accumulate:
/// `[ok:1][old note root:32][old null root:32][new note root:32][new null root:32][n:2][nullifiers: n*32]`
const HEADER_LEN: usize = 1 + 4 * HASH_LEN + 2;

fn invalid() -> WorkOutput {
    vec![0u8].into()
}

impl Service for Privacy {
    /// Verify the builder's folded proof against its public inputs and pass the state
    /// change on. Until step 0 lands there is no verifier: every item is invalid.
    fn refine(
        _core_index: CoreIndex,
        _item_index: usize,
        _service_id: ServiceId,
        _payload: WorkPayload,
        _package_hash: WorkPackageHash,
    ) -> WorkOutput {
        invalid()
    }

    /// Old roots must match the stored ones and every nullifier must be new; then write
    /// the nullifiers and the new roots. Anything else drops the item.
    fn accumulate(_slot: Slot, _service_id: ServiceId, _item_count: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let rec = match item {
                AccumulateItem::WorkItem(r) => r,
                _ => continue, // transfers (shield) come in step 1
            };
            let out = match rec.result {
                Ok(o) => o.0,
                Err(_) => continue,
            };
            if out.len() < HEADER_LEN || out[0] != 1u8 {
                continue;
            }
            let old_note = &out[1..1 + HASH_LEN];
            let old_null = &out[1 + HASH_LEN..1 + 2 * HASH_LEN];
            let new_note = &out[1 + 2 * HASH_LEN..1 + 3 * HASH_LEN];
            let new_null = &out[1 + 3 * HASH_LEN..1 + 4 * HASH_LEN];
            let n = u16::from_le_bytes([out[1 + 4 * HASH_LEN], out[2 + 4 * HASH_LEN]]) as usize;
            if out.len() < HEADER_LEN + n * HASH_LEN {
                continue;
            }
            let stored_note = get_storage(KEY_NOTE_ROOT).unwrap_or_default();
            let stored_null = get_storage(KEY_NULL_ROOT).unwrap_or_default();
            if (!stored_note.is_empty() && stored_note.as_slice() != old_note)
                || (!stored_null.is_empty() && stored_null.as_slice() != old_null)
            {
                continue; // stale root: another package moved the tree first
            }
            let nullifiers: Vec<&[u8]> = (0..n)
                .map(|i| &out[HEADER_LEN + i * HASH_LEN..HEADER_LEN + (i + 1) * HASH_LEN])
                .collect();
            if nullifiers.iter().any(|nf| get_storage(&key_for(nf)).is_some()) {
                continue; // double spend
            }
            for nf in nullifiers {
                set_storage(&key_for(nf), &[1u8]).ok();
            }
            set_storage(KEY_NOTE_ROOT, new_note).ok();
            set_storage(KEY_NULL_ROOT, new_null).ok();
        }
        None
    }
}

fn key_for(nullifier: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(3 + HASH_LEN);
    key.extend_from_slice(b"nf:");
    key.extend_from_slice(nullifier);
    key
}
