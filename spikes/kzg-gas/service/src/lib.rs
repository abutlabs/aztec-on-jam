//! Spike: EIP-4844's KZG checks in refine. Payload [tag][kind][data]:
//!   kind 0: the 0x0a precompile's 192-byte input (Aztec's BlobLib.sol, once per epoch proof)
//!   kind 1: [n][n × (blob 131,072 || C 48 || pi 48)], each blob checked against its
//!           commitment (verify_blob_kzg_proof; Ethereum's consensus does this per blob)
//! Refine outputs [tag][1 all valid | 0 otherwise]; accumulate stores it at "v:" ++ tag.
#![no_std]
#![no_main]

extern crate alloc;

use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Spike);
struct Spike;

const ITEM: usize = kzg_verify::BYTES_PER_BLOB + 96;

fn blobs(d: &[u8]) -> Option<bool> {
    let n = *d.first()? as usize;
    let items = d.get(1..)?;
    if n == 0 || items.len() != n * ITEM {
        return None;
    }
    for it in items.chunks_exact(ITEM) {
        let (blob, rest) = it.split_at(kzg_verify::BYTES_PER_BLOB);
        if kzg_verify::verify_blob_kzg_proof(blob, rest[..48].try_into().ok()?, rest[48..].try_into().ok()?) != Ok(true) {
            return Some(false);
        }
    }
    Some(true)
}

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.first().copied().unwrap_or(0);
        let ok = match (p.get(1), p.get(2..)) {
            (Some(0), Some(input)) => kzg_verify::point_evaluation(input) == Ok(true),
            (Some(1), Some(d)) => blobs(d).unwrap_or(false),
            _ => false,
        };
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
