//! Aztec v6's Rollup as a JAM service (docs/aztec/SERVICE.md, #49): the checkpoint path.
//!
//! Configuration is compiled in (build.rs: `cfg.bin`, written by `measure.sh` or a deployment), so no
//! work item can set it: [genesis archive 32][Aztec genesis unix time u64][Aztec slot seconds
//! u64][epoch length in Aztec slots u64][mana minimum fee u128][mana limit 32], big-endian.
//!
//! Work item kind 1, a checkpoint (`ProposeLib.propose`):
//!   payload [1][tag u8][header 348, ProposedHeaderLib's packing][archive 32][n u8][n × (C 48 ‖ proof 48)]
//!   extrinsic j: blob j (131,072 bytes)
//!   refine: the header's stateless checks (`aztec_l1::check_header`: fields in range, coinbase,
//!     mana, blobsHash against the commitments, no DA fee), the blobs against their commitments
//!     (EIP-4844's batch check), the blobs exported as segments; output
//!     [1][tag][header hash 32][archive 32][last archive 32][slot u64][timestamp u64][inbox hash 32]
//!     [out hash 32][fee per L2 gas u128][n u8][n × C 48], or [0][tag][reason] if a check fails
//!   accumulate (`validateHeader`'s stateful half): the parent archive is the pending tip, the slot
//!     is after the tip's and not ahead of the Aztec clock (JAM's timeslot), the timestamp is the
//!     slot's, the fee is the minimum, the inbox hash is the current one; then `cp:<n>` and
//!     `tip:pending` are written and the epoch's blob commitments hash is advanced.
//! Every item's verdict (1 accepted, or the reason codes below) is stored at "v:" ++ its tag. A
//! refused item still exports its declared number of segments (zeros), because a work item that
//! exports a different number has its output replaced by an error (GP 0.8.0
//! `work_packages_and_reports.tex:215`) and its reason would be lost.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use aztec_l1::{blob_commitments_hash, check_header, parse_blob_commitments, Header, HEADER_LEN};
use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::refine;
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(4 * 1024 * 1024);

declare_service!(Settlement);
struct Settlement;

const CFG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cfg.bin"));
/// JAM's Common Era, 2025-01-01 12:00 UTC (GP 0.8.0 `overview.tex:91`), and its 6-second slots.
const JAM_COMMON_ERA: u64 = 1_735_732_800;
const JAM_SLOT_SECONDS: u64 = 6;

struct Cfg {
    genesis_archive: [u8; 32],
    genesis_time: u64,
    slot_seconds: u64,
    epoch_slots: u64,
    min_fee: u128,
    mana_limit: [u8; 32],
}

fn cfg() -> Cfg {
    let u = |at: usize| u64::from_be_bytes(CFG[at..at + 8].try_into().unwrap());
    Cfg {
        genesis_archive: CFG[..32].try_into().unwrap(),
        genesis_time: u(32),
        slot_seconds: u(40),
        epoch_slots: u(48),
        min_fee: u128::from_be_bytes(CFG[56..72].try_into().unwrap()),
        mana_limit: CFG[72..104].try_into().unwrap(),
    }
}

// Refused in refine: the first byte after 0.
const BAD_PAYLOAD: u8 = 1;
const BAD_HEADER: u8 = 2;
const BAD_BLOBS: u8 = 3;
// Accumulate's verdicts.
const ACCEPTED: u8 = 1;
const WRONG_PARENT: u8 = 10;
const SLOT_NOT_AFTER_TIP: u8 = 11;
const SLOT_IN_FUTURE: u8 = 12;
const WRONG_TIMESTAMP: u8 = 13;
const WRONG_FEE: u8 = 14;
const WRONG_INBOX: u8 = 15;

fn checkpoint(p: &[u8]) -> Result<Vec<u8>, u8> {
    let c = cfg();
    let header = Header::decode(p.get(..HEADER_LEN).ok_or(BAD_PAYLOAD)?).ok_or(BAD_PAYLOAD)?;
    let archive: [u8; 32] = p.get(HEADER_LEN..HEADER_LEN + 32).ok_or(BAD_PAYLOAD)?.try_into().unwrap();
    let blobs = p.get(HEADER_LEN + 32..).ok_or(BAD_PAYLOAD)?;
    let n = *blobs.first().ok_or(BAD_PAYLOAD)? as usize;
    if n == 0 || blobs.len() != 1 + 96 * n {
        return Err(BAD_PAYLOAD);
    }
    let items: Vec<&[u8]> = blobs[1..].chunks_exact(96).collect();
    let mut prefixed = alloc::vec![n as u8];
    items.iter().for_each(|it| prefixed.extend_from_slice(&it[..48]));
    let commitments = parse_blob_commitments(&prefixed).ok_or(BAD_PAYLOAD)?;
    check_header(&header, &aztec_l1::blobs_hash(&commitments), &c.mana_limit).map_err(|_| BAD_HEADER)?;
    let slot = header.slot().ok_or(BAD_HEADER)?;
    let data: Vec<Vec<u8>> = (0..n).map(refine::extrinsic).collect::<Option<_>>().ok_or(BAD_BLOBS)?;
    let proofs: Vec<[u8; 48]> = items.iter().map(|it| it[48..].try_into().unwrap()).collect();
    let refs: Vec<&[u8]> = data.iter().map(|b| b.as_slice()).collect();
    if kzg_verify::verify_blob_kzg_proof_batch(&refs, &commitments, &proofs) != Ok(true) {
        return Err(BAD_BLOBS);
    }
    for blob in &data {
        for seg in blob.chunks(SEGMENT_LEN) {
            refine::export_slice(seg).map_err(|_| BAD_BLOBS)?;
        }
    }
    let mut out = alloc::vec![];
    out.extend_from_slice(&header.hash());
    out.extend_from_slice(&archive);
    out.extend_from_slice(&header.last_archive_root);
    out.extend_from_slice(&slot.to_le_bytes());
    out.extend_from_slice(&header.timestamp.to_le_bytes());
    out.extend_from_slice(&header.inbox_rolling_hash);
    out.extend_from_slice(&header.out_hash);
    out.extend_from_slice(&header.fee_per_l2_gas.to_le_bytes());
    out.push(n as u8);
    commitments.iter().for_each(|cm| out.extend_from_slice(cm));
    Ok(out)
}

/// The pending tip: [number u64][archive 32][slot u64][epoch u64][blob commitments hash 32].
struct Tip {
    number: u64,
    archive: [u8; 32],
    slot: u64,
    epoch: u64,
    blob_commitments_hash: [u8; 32],
}

fn tip(c: &Cfg) -> Tip {
    match get_storage(b"tip:pending") {
        Some(t) if t.len() == 88 => Tip {
            number: u64::from_le_bytes(t[..8].try_into().unwrap()),
            archive: t[8..40].try_into().unwrap(),
            slot: u64::from_le_bytes(t[40..48].try_into().unwrap()),
            epoch: u64::from_le_bytes(t[48..56].try_into().unwrap()),
            blob_commitments_hash: t[56..88].try_into().unwrap(),
        },
        _ => Tip { number: 0, archive: c.genesis_archive, slot: 0, epoch: 0, blob_commitments_hash: [0; 32] },
    }
}

fn accept(o: &[u8], timeslot: u32) -> u8 {
    let c = cfg();
    let t = tip(&c);
    // o is refine's output after [1][tag]
    let w = |at: usize| -> [u8; 32] { o[at..at + 32].try_into().unwrap() };
    let u = |at: usize| u64::from_le_bytes(o[at..at + 8].try_into().unwrap());
    let (header_hash, archive, last_archive) = (w(0), w(32), w(64));
    let (slot, timestamp) = (u(96), u(104));
    let (inbox, out_hash) = (w(112), w(144));
    let fee = u128::from_le_bytes(o[176..192].try_into().unwrap());
    let n = o[192] as usize;
    let commitments: Vec<[u8; 48]> = o[193..193 + 48 * n].chunks_exact(48).map(|x| x.try_into().unwrap()).collect();
    if last_archive != t.archive {
        return WRONG_PARENT;
    }
    if slot <= t.slot {
        return SLOT_NOT_AFTER_TIP;
    }
    let now = JAM_COMMON_ERA + timeslot as u64 * JAM_SLOT_SECONDS;
    let current_slot = now.saturating_sub(c.genesis_time) / c.slot_seconds;
    if slot > current_slot {
        return SLOT_IN_FUTURE;
    }
    if timestamp != c.genesis_time + slot * c.slot_seconds {
        return WRONG_TIMESTAMP;
    }
    if fee != c.min_fee {
        return WRONG_FEE;
    }
    let current_inbox = get_storage(b"inbox:hash").map(|v| <[u8; 32]>::try_from(&v[..]).unwrap()).unwrap_or([0; 32]);
    if inbox != current_inbox {
        return WRONG_INBOX;
    }
    let epoch = slot / c.epoch_slots;
    let first_of_epoch = t.number == 0 || epoch > t.epoch;
    let bch = blob_commitments_hash(&t.blob_commitments_hash, &commitments, first_of_epoch);
    let number = t.number + 1;
    // cp:<n>: [header hash 32][archive 32][slot u64][out hash 32][inbox hash 32][blob commitments hash 32]
    let mut log = Vec::with_capacity(168);
    log.extend_from_slice(&header_hash);
    log.extend_from_slice(&archive);
    log.extend_from_slice(&slot.to_le_bytes());
    log.extend_from_slice(&out_hash);
    log.extend_from_slice(&inbox);
    log.extend_from_slice(&bch);
    let mut key = b"cp:".to_vec();
    key.extend_from_slice(&number.to_le_bytes());
    set_storage(&key, &log).ok();
    let mut tip = Vec::with_capacity(88);
    tip.extend_from_slice(&number.to_le_bytes());
    tip.extend_from_slice(&archive);
    tip.extend_from_slice(&slot.to_le_bytes());
    tip.extend_from_slice(&epoch.to_le_bytes());
    tip.extend_from_slice(&bch);
    set_storage(b"tip:pending", &tip).ok();
    ACCEPTED
}

impl Service for Settlement {
    fn refine(_core: CoreIndex, item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.get(1).copied().unwrap_or(0);
        let out = match (p.first(), p.get(2..)) {
            (Some(1), Some(rest)) => checkpoint(rest),
            _ => Err(BAD_PAYLOAD),
        };
        match out {
            Ok(o) => [&[1u8, tag][..], &o].concat().into(),
            Err(reason) => {
                let declared = refine::work_item_summary(item).map_or(0, |s| s.export_count as usize);
                let zero = [0u8; SEGMENT_LEN];
                (0..declared).for_each(|_| { refine::export_slice(&zero).ok(); });
                alloc::vec![0u8, tag, reason].into()
            }
        }
    }

    fn accumulate(timeslot: Slot, _service: ServiceId, _items: usize) -> Option<Hash> {
        for item in accumulate_items() {
            let AccumulateItem::WorkItem(rec) = item else { continue };
            let Ok(out) = rec.result else { continue };
            let o = out.0;
            let tag = o.get(1).copied().unwrap_or(0);
            let verdict = match o.first() {
                Some(1) if o.len() >= 195 && o.len() == 195 + 48 * o[194] as usize => accept(&o[2..], timeslot),
                _ => o.get(2).copied().unwrap_or(0),
            };
            set_storage(&[b'v', b':', tag], &[verdict]).ok();
        }
        None
    }
}
