//! Spike: EIP-4844's KZG checks in refine. Payload [tag][kind][data]:
//!   kind 0: the 0x0a precompile's 192-byte input (Aztec's BlobLib.sol, once per epoch proof)
//!   kind 1: [n][n × (blob 131,072 || C 48 || pi 48)], each blob checked against its
//!           commitment (verify_blob_kzg_proof; Ethereum's consensus does this per blob)
//!   kind 3: [n][export 0|1][n × (C 48 || pi 48)], blob i in the work-item's extrinsic i (JAM-NP
//!           caps the package, payloads included, at 200 KiB; extrinsics travel beside it);
//!           with export 1, each checked blob is exported as 32 segments (28 days in the DA)
//!   kind 4: [n][n × (C 48 || pi 48)], blob i in extrinsic i, all checked at once
//!           (verify_blob_kzg_proof_batch: one pairing for the n)
//!   kind 5: [n][C 48 || pi 48], the blob imported as the work-item's n segments (exported by
//!           an earlier package, kept 28 days in the DA), its first 131,072 bytes checked
//! Refine outputs [tag][1 all valid | 0 otherwise]; accumulate stores it at "v:" ++ tag.
#![no_std]
#![no_main]

extern crate alloc;

extern crate alloc as alloc_crate;
use alloc_crate::vec::Vec;
use jam_pvm_common::accumulate::{accumulate_items, set_storage};
use jam_pvm_common::refine;
use jam_pvm_common::jam_types::SEGMENT_LEN;
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

fn extrinsic_blobs(d: &[u8]) -> Option<bool> {
    let (n, export) = (*d.first()? as usize, *d.get(1)? == 1);
    let proofs = d.get(2..)?;
    if n == 0 || proofs.len() != n * 96 {
        return None;
    }
    for (i, cp) in proofs.chunks_exact(96).enumerate() {
        let blob = refine::extrinsic(i)?;
        if kzg_verify::verify_blob_kzg_proof(&blob, cp[..48].try_into().ok()?, cp[48..].try_into().ok()?) != Ok(true) {
            return Some(false);
        }
        if export {
            for seg in blob.chunks(SEGMENT_LEN) {
                refine::export_slice(seg).ok()?;
            }
        }
    }
    Some(true)
}

fn extrinsic_batch(d: &[u8]) -> Option<bool> {
    let n = *d.first()? as usize;
    let proofs = d.get(1..)?;
    if proofs.len() != n * 96 {
        return None;
    }
    let blobs: Vec<Vec<u8>> = (0..n).map(refine::extrinsic).collect::<Option<_>>()?;
    let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
    let cs: Vec<[u8; 48]> = proofs.chunks_exact(96).map(|c| c[..48].try_into().unwrap()).collect();
    let pis: Vec<[u8; 48]> = proofs.chunks_exact(96).map(|c| c[48..].try_into().unwrap()).collect();
    Some(kzg_verify::verify_blob_kzg_proof_batch(&refs, &cs, &pis) == Ok(true))
}

fn imported_blob(d: &[u8]) -> Option<bool> {
    let n = *d.first()? as usize;
    let cp = d.get(1..97)?;
    let mut blob = Vec::with_capacity(n * SEGMENT_LEN);
    for i in 0..n {
        blob.extend_from_slice(refine::import(i)?.as_ref());
    }
    blob.truncate(kzg_verify::BYTES_PER_BLOB);
    Some(kzg_verify::verify_blob_kzg_proof(&blob, cp[..48].try_into().ok()?, cp[48..].try_into().ok()?) == Ok(true))
}

impl Service for Spike {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.first().copied().unwrap_or(0);
        let ok = match (p.get(1), p.get(2..)) {
            (Some(0), Some(input)) => kzg_verify::point_evaluation(input) == Ok(true),
            (Some(1), Some(d)) => blobs(d).unwrap_or(false),
            (Some(3), Some(d)) => extrinsic_blobs(d).unwrap_or(false),
            (Some(4), Some(d)) => extrinsic_batch(d).unwrap_or(false),
            (Some(5), Some(d)) => imported_blob(d).unwrap_or(false),
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
