//! Aztec v6's Rollup as a JAM service (docs/aztec/SERVICE.md, #49): the checkpoint path.
//!
//! Configuration is compiled in (build.rs: `cfg.bin`, written by `measure.sh` or a deployment), so no
//! work item can set it: [genesis archive 32][Aztec genesis unix time u64][Aztec slot seconds
//! u64][epoch length in Aztec slots u64][mana minimum fee u128][mana limit 32][chain id 32][version 32],
//! big-endian.
//!
//! The Inbox (`Inbox.sol`, #50): an L1→L2 message is a deferred transfer to this service whose
//! memo is [recipient 32][content 32][secret hash 32][version 32]; its L1 sender is the sending
//! service (its id as a 20-byte address) on `cfg`'s chain id. Accumulate hashes it into a leaf
//! (`aztec_l1::l1_to_l2_leaf`) and absorbs it into the current bucket's rolling hash, opening a
//! new bucket each JAM timeslot, as Ethereum's Inbox does each L1 block, or when one holds 256.
//! A message with a field out of range or another version is refused and counted; the transfer
//! has already moved its balance, which stays with this service. Not ported: the 4,096-bucket
//! ring and its eviction once a bucket is proven consumed (buckets are kept).
//!
//! Work item kind 1, a checkpoint (`ProposeLib.propose`):
//!   payload [1][tag u8][header 348, ProposedHeaderLib's packing][archive 32][bucket hint u64 LE]
//!     [n u8][n × (C 48 ‖ proof 48)]
//!   extrinsic j: blob j (131,072 bytes)
//!   refine: the header's stateless checks (`aztec_l1::check_header`: fields in range, coinbase,
//!     mana, blobsHash against the commitments, no DA fee), the blobs against their commitments
//!     (EIP-4844's batch check), the blobs exported as segments; output
//!     [1][tag][header hash 32][archive 32][last archive 32][slot u64][timestamp u64][inbox hash 32]
//!     [out hash 32][fee per L2 gas u128][bucket hint u64][n u8][n × C 48], or [0][tag][reason]
//!   accumulate (`validateHeader`'s stateful half): the parent archive is the pending tip, the slot
//!     is after the tip's and not ahead of the Aztec clock (JAM's timeslot), the timestamp is the
//!     slot's, the fee is the minimum, and the inbox is consumed as `validateInboxConsumption`
//!     requires; then `cp:<n>` and `tip:pending` are written and the epoch's blob commitments hash
//!     is advanced.
//! Every item's verdict (1 accepted, or the reason codes below) is stored at "v:" ++ its tag. A
//! refused item still exports its declared number of segments (zeros), because a work item that
//! exports a different number has its output replaced by an error (GP 0.8.0
//! `work_packages_and_reports.tex:215`) and its reason would be lost.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use aztec_l1::{blob_commitments_hash, check_header, inbox_rolling_hash, is_field, l1_to_l2_leaf, parse_blob_commitments, Header, HEADER_LEN};
use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::refine;
use jam_pvm_common::{declare_service, Service};

polkavm_derive::min_stack_size!(4 * 1024 * 1024);

declare_service!(Settlement);
struct Settlement;

const CFG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cfg.bin"));
const _: () = assert!(CFG.len() == 168, "cfg.bin must be 168 bytes");
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
    chain_id: [u8; 32],
    version: [u8; 32],
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
        chain_id: CFG[104..136].try_into().unwrap(),
        version: CFG[136..168].try_into().unwrap(),
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
const INBOX_NO_BUCKET: u8 = 16;
const INBOX_STILL_MUTABLE: u8 = 17;
const INBOX_BEHIND_PARENT: u8 = 18;
const INBOX_TOO_MANY: u8 = 19;
const INBOX_UNCONSUMED: u8 = 20;
/// `Constants.MAX_L1_TO_L2_MSGS_PER_BLOCK` (a bucket) and `_PER_CHECKPOINT`.
const MAX_MSGS_PER_BUCKET: u32 = 256;
const MAX_MSGS_PER_CHECKPOINT: u64 = 1024;

fn checkpoint(p: &[u8]) -> Result<Vec<u8>, u8> {
    let c = cfg();
    let header = Header::decode(p.get(..HEADER_LEN).ok_or(BAD_PAYLOAD)?).ok_or(BAD_PAYLOAD)?;
    let archive: [u8; 32] = p.get(HEADER_LEN..HEADER_LEN + 32).ok_or(BAD_PAYLOAD)?.try_into().unwrap();
    let hint: [u8; 8] = p.get(HEADER_LEN + 32..HEADER_LEN + 40).ok_or(BAD_PAYLOAD)?.try_into().unwrap();
    let blobs = p.get(HEADER_LEN + 40..).ok_or(BAD_PAYLOAD)?;
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
    out.extend_from_slice(&hint);
    out.push(n as u8);
    commitments.iter().for_each(|cm| out.extend_from_slice(cm));
    Ok(out)
}

/// The pending tip: [number u64][archive 32][slot u64][epoch u64][blob commitments hash 32]
/// [inbox messages consumed u64].
struct Tip {
    number: u64,
    archive: [u8; 32],
    slot: u64,
    epoch: u64,
    blob_commitments_hash: [u8; 32],
    consumed: u64,
}

fn tip(c: &Cfg) -> Tip {
    let u = |t: &[u8], at: usize| u64::from_le_bytes(t[at..at + 8].try_into().unwrap());
    match get_storage(b"tip:pending") {
        Some(t) if t.len() == 96 => Tip {
            number: u(&t, 0),
            archive: t[8..40].try_into().unwrap(),
            slot: u(&t, 40),
            epoch: u(&t, 48),
            blob_commitments_hash: t[56..88].try_into().unwrap(),
            consumed: u(&t, 88),
        },
        _ => Tip { number: 0, archive: c.genesis_archive, slot: 0, epoch: 0, blob_commitments_hash: [0; 32], consumed: 0 },
    }
}

/// An Inbox bucket (`IInbox.InboxBucket`), at "inbox:b:" ++ its sequence number:
/// [rolling hash 32][total messages so far u64][unix time it opened u64][messages in it u32].
/// Bucket 0 is the genesis bucket, never stored: zero hash, zero messages.
#[derive(Clone, Copy)]
struct Bucket {
    rolling_hash: [u8; 32],
    total: u64,
    time: u64,
    count: u32,
}

fn key(prefix: &[u8], n: u64) -> Vec<u8> {
    [prefix, &n.to_le_bytes()[..]].concat()
}

fn bucket(seq: u64) -> Option<Bucket> {
    if seq == 0 {
        return Some(Bucket { rolling_hash: [0; 32], total: 0, time: 0, count: 0 });
    }
    let b = get_storage(&key(b"inbox:b:", seq)).filter(|b| b.len() == 52)?;
    Some(Bucket {
        rolling_hash: b[..32].try_into().unwrap(),
        total: u64::from_le_bytes(b[32..40].try_into().unwrap()),
        time: u64::from_le_bytes(b[40..48].try_into().unwrap()),
        count: u32::from_le_bytes(b[48..52].try_into().unwrap()),
    })
}

fn inbox_seq() -> u64 {
    get_storage(b"inbox:seq").map_or(0, |v| u64::from_le_bytes(v[..8].try_into().unwrap()))
}

fn unix_time(timeslot: u32) -> u64 {
    JAM_COMMON_ERA + timeslot as u64 * JAM_SLOT_SECONDS
}

/// `Inbox.sendL2Message` and `_absorbIntoBucket`, for one incoming transfer.
fn absorb(source: u32, memo: &[u8], now: u64, c: &Cfg) -> bool {
    let w = |at: usize| -> [u8; 32] { memo[at..at + 32].try_into().unwrap() };
    let (recipient, content, secret_hash, version) = (w(0), w(32), w(64), w(96));
    if !is_field(&recipient) || !is_field(&content) || !is_field(&secret_hash) || version != c.version {
        return false;
    }
    let mut seq = inbox_seq();
    let mut b = bucket(seq).unwrap();
    if seq == 0 || b.time < now || b.count == MAX_MSGS_PER_BUCKET {
        seq += 1;
        b = Bucket { time: now, count: 0, ..b };
        set_storage(b"inbox:seq", &seq.to_le_bytes()).ok();
    }
    let index = b.total;
    let leaf = l1_to_l2_leaf(source, &c.chain_id, &recipient, &version, &content, &secret_hash, index);
    b.rolling_hash = inbox_rolling_hash(&b.rolling_hash, &leaf);
    b.total += 1;
    b.count += 1;
    let stored = [&b.rolling_hash[..], &b.total.to_le_bytes(), &b.time.to_le_bytes(), &b.count.to_le_bytes()].concat();
    set_storage(&key(b"inbox:b:", seq), &stored).ok();
    // for Aztec's node: the leaf and the message, at its index
    let msg = [&leaf[..], &source.to_le_bytes(), &recipient, &content, &secret_hash].concat();
    set_storage(&key(b"inbox:m:", index), &msg).ok();
    true
}

/// `ProposeLib.validateInboxConsumption`: the checkpoint consumes the Inbox up to bucket `hint`,
/// whose rolling hash its header carries. Returns the total consumed.
fn consume(inbox: &[u8; 32], hint: u64, slot: u64, parent: u64, now: u64, c: &Cfg) -> Result<u64, u8> {
    let seq = inbox_seq();
    let b = if hint <= seq { bucket(hint) } else { None }.ok_or(INBOX_NO_BUCKET)?;
    if &b.rolling_hash != inbox {
        return Err(WRONG_INBOX);
    }
    if !(hint == 0 || b.time < now || b.count == MAX_MSGS_PER_BUCKET) {
        return Err(INBOX_STILL_MUTABLE);
    }
    if b.total < parent {
        return Err(INBOX_BEHIND_PARENT);
    }
    if b.total - parent > MAX_MSGS_PER_CHECKPOINT {
        return Err(INBOX_TOO_MANY);
    }
    if hint < seq {
        let next = bucket(hint + 1).ok_or(INBOX_NO_BUCKET)?;
        // `TimeLib.getInboxCutoffTimestamp`: the previous slot's start, less one (JAM) slot
        let cutoff = (c.genesis_time + slot.saturating_sub(1) * c.slot_seconds).saturating_sub(JAM_SLOT_SECONDS);
        if !(next.time > cutoff || next.total - parent > MAX_MSGS_PER_CHECKPOINT) {
            return Err(INBOX_UNCONSUMED);
        }
    }
    Ok(b.total)
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
    let hint = u(192);
    let n = o[200] as usize;
    let commitments: Vec<[u8; 48]> = o[201..201 + 48 * n].chunks_exact(48).map(|x| x.try_into().unwrap()).collect();
    if last_archive != t.archive {
        return WRONG_PARENT;
    }
    if slot <= t.slot {
        return SLOT_NOT_AFTER_TIP;
    }
    let now = unix_time(timeslot);
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
    let consumed = match consume(&inbox, hint, slot, t.consumed, now, &c) {
        Ok(total) => total,
        Err(e) => return e,
    };
    let epoch = slot / c.epoch_slots;
    let first_of_epoch = t.number == 0 || epoch > t.epoch;
    let bch = blob_commitments_hash(&t.blob_commitments_hash, &commitments, first_of_epoch);
    let number = t.number + 1;
    // cp:<n>: [header hash 32][archive 32][slot u64][out hash 32][inbox hash 32][blob commitments hash 32]
    // [inbox messages consumed u64][bucket u64]
    let mut log = Vec::with_capacity(184);
    log.extend_from_slice(&header_hash);
    log.extend_from_slice(&archive);
    log.extend_from_slice(&slot.to_le_bytes());
    log.extend_from_slice(&out_hash);
    log.extend_from_slice(&inbox);
    log.extend_from_slice(&bch);
    log.extend_from_slice(&consumed.to_le_bytes());
    log.extend_from_slice(&hint.to_le_bytes());
    set_storage(&key(b"cp:", number), &log).ok();
    let mut tip = Vec::with_capacity(96);
    tip.extend_from_slice(&number.to_le_bytes());
    tip.extend_from_slice(&archive);
    tip.extend_from_slice(&slot.to_le_bytes());
    tip.extend_from_slice(&epoch.to_le_bytes());
    tip.extend_from_slice(&bch);
    tip.extend_from_slice(&consumed.to_le_bytes());
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
        let c = cfg();
        for item in accumulate_items() {
            let rec = match item {
                AccumulateItem::Transfer(t) => {
                    if !absorb(t.source, &t.memo.0[..], unix_time(timeslot), &c) {
                        let n = get_storage(b"inbox:refused").map_or(0u64, |v| u64::from_le_bytes(v[..8].try_into().unwrap()));
                        set_storage(b"inbox:refused", &(n + 1).to_le_bytes()).ok();
                    }
                    continue;
                }
                AccumulateItem::WorkItem(rec) => rec,
            };
            let Ok(out) = rec.result else { continue };
            let o = out.0;
            let tag = o.get(1).copied().unwrap_or(0);
            let verdict = match o.first() {
                Some(1) if o.len() >= 203 && o.len() == 203 + 48 * o[202] as usize => accept(&o[2..], timeslot),
                _ => o.get(2).copied().unwrap_or(0),
            };
            set_storage(&[b'v', b':', tag], &[verdict]).ok();
        }
        None
    }
}
