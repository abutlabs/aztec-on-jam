//! #26's skeleton of the settlement service (docs/aztec/SERVICE.md): the epoch-proof path.
//! Payload: [vk_len u16][vk][n u16][n × 32 public inputs][proof], a plain UltraHonk proof with
//! a Keccak transcript, as Aztec's root rollup. Its first two public inputs stand for
//! `previous_archive_root` and `end_archive_root`.
//!   refine: verify the proof (it reads no state); output [1][previous][end], or [0]
//!   accumulate: `tip` is the proven archive root. A verified proof advances it only if its
//!     previous root equals the tip (the first accepted proof anchors the chain, standing in for
//!     a genesis value set at deployment). Records `proven` (a count) and `last`: 1 advanced,
//!     2 stale (previous is not the tip), 0 the proof failed.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::{declare_service, Service};
use ultrahonk_no_std::{verify, ProofType};

polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Settlement);
struct Settlement;

fn le16(b: &[u8]) -> Option<usize> { Some(u16::from_le_bytes(b.get(..2)?.try_into().ok()?) as usize) }

fn check(p: &[u8]) -> Option<([u8; 32], [u8; 32])> {
    let vk_len = le16(p)?;
    let vk = p.get(2..2 + vk_len)?;
    let at = 2 + vk_len;
    let n = le16(p.get(at..)?)?;
    let pubs: Vec<[u8; 32]> = p.get(at + 2..at + 2 + 32 * n)?.chunks_exact(32).map(|c| c.try_into().unwrap()).collect();
    let proof = p.get(at + 2 + 32 * n..)?;
    verify::<()>(vk, &ProofType::Plain(proof.into()), &pubs).ok()?;
    Some((*pubs.first()?, *pubs.get(1)?))
}

impl Service for Settlement {
    fn refine(_core: CoreIndex, _item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        match check(&payload.take()) {
            Some((prev, end)) => {
                let mut out = alloc::vec![1u8];
                out.extend_from_slice(&prev);
                out.extend_from_slice(&end);
                out.into()
            }
            None => alloc::vec![0u8].into(),
        }
    }

    fn accumulate(_slot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let AccumulateItem::WorkItem(rec) = item else { continue };
            let Ok(out) = rec.result else { continue };
            let out = out.0;
            if out.len() != 65 || out[0] != 1 {
                set_storage(b"last", &[0u8]).ok();
                continue;
            }
            let (prev, end) = (&out[1..33], &out[33..65]);
            let tip = get_storage(b"tip");
            if tip.as_deref().map_or(true, |t| t == prev) {
                set_storage(b"tip", end).ok();
                let n = get_storage(b"proven").map(|v| u32::from_le_bytes(v[..4].try_into().unwrap())).unwrap_or(0);
                set_storage(b"proven", &(n + 1).to_le_bytes()).ok();
                set_storage(b"last", &[1u8]).ok();
            } else {
                set_storage(b"last", &[2u8]).ok();
            }
        }
        None
    }
}
