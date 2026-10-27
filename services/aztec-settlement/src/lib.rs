//! Aztec v6's Rollup as a JAM service (docs/aztec/SERVICE.md, #49): the checkpoint path.
//!
//! Configuration is compiled in (build.rs: `cfg.bin`, written by `measure.sh` or a deployment), so no
//! work item can set it: [genesis archive 32][Aztec genesis unix time u64][Aztec slot seconds
//! u64][epoch length in Aztec slots u64][mana minimum fee u128][mana limit 32][chain id 32][version 32]
//! [epoch proof's vk sha256 32][vk tree root 32][protocol contracts hash 32][proof submission epochs
//! u64][rollup address 20 (the signatures' EIP-712 verifying contract)][sample seed 32][committee size
//! u8][16 × address 20, the first `size` used], big-endian.
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
//!     [n u8][n × (C 48 ‖ proof 48)][fee asset price modifier 32][signature indices len u16 LE][..]
//!     [signatures or addresses len u16 LE][..][signers u16 LE][signers × 20][proposer's signature 65]
//!   refine also runs `ValidatorSelectionLib.verifyProposer` against `cfg`'s committee (#54): the
//!     attestations reconstruct it, the slot's proposer signed the payload digest and the
//!     attestations-and-signers digest. An empty committee skips it, as the Rollup does.
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
//! Work item kind 2, an epoch proof (`EpochProofLib.submitEpochRootProof`, #51):
//!   payload [2][tag u8][start u64 LE][end u64 LE][vk_len u16 LE][vk][113 × 32 public inputs]
//!     [proof_len u32 LE][proof][batched blob opening 192]
//!   refine: the root rollup proof (plain UltraHonk, Keccak transcript) against its public
//!     inputs, the batched blob opening (EIP-4844's point evaluation), and the last six public
//!     inputs against that opening; output [2][tag][start][end][vk sha256 32][the first 107
//!     public inputs × 32]
//!   accumulate (`assertAcceptable` and `computeEpochProofPublicInputs`, by comparison): the vk is
//!     `cfg`'s; start..end is one epoch, from its first checkpoint, building on the proven tip, at
//!     most 32, within the proof window; every public input equals what Ethereum's Rollup would
//!     assemble from stored checkpoints and `cfg`. Then `tip:proven` advances and the epoch's out
//!     hash goes to `outbox:<epoch>` (unless empty). Not ported: attestations (#54), pruning
//!     unproven checkpoints, prover rewards.
//! Every item's verdict (1 accepted, or the reason codes below) is stored at "v:" ++ its tag. A
//! refused item still exports its declared number of segments (zeros), because a work item that
//! exports a different number has its output replaced by an error (GP 0.8.0
//! `work_packages_and_reports.tex:215`) and its reason would be lost.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use aztec_l1::signatures;
use aztec_l1::{
    address_to_field, blob_commitments_hash, blob_public_inputs, check_header, epoch_pi, inbox_rolling_hash, is_field,
    l1_to_l2_leaf, parse_blob_commitments, sha256_to_field, Header, EMPTY_EPOCH_OUT_HASH, HEADER_LEN,
    MAX_CHECKPOINTS_PER_EPOCH, ROOT_ROLLUP_PUBLIC_INPUTS,
};
use ultrahonk_no_std::{verify, ProofType};
use jam_pvm_common::accumulate::{accumulate_items, get_storage, set_storage};
use jam_pvm_common::jam_types::*;
use jam_pvm_common::refine;
use jam_pvm_common::{declare_service, Service};

// BN254 pairings overflow polkavm's default stack (spikes/epoch-proof)
polkavm_derive::min_stack_size!(8 * 1024 * 1024);

declare_service!(Settlement);
struct Settlement;

const CFG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cfg.bin"));
const _: () = assert!(CFG.len() == 645 || CFG.len() == 653, "cfg.bin must be 645 bytes, or 653 with the JAM era");
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
    vk_hash: [u8; 32],
    vk_tree_root: [u8; 32],
    protocol_contracts_hash: [u8; 32],
    proof_submission_epochs: u64,
    rollup: [u8; 20],
    seed: [u8; 32],
    committee: Vec<[u8; 20]>,
    /// Unix time of JAM timeslot 0: the common era, or the net's own (a devnet paced from slot 0).
    jam_era: u64,
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
        vk_hash: CFG[168..200].try_into().unwrap(),
        vk_tree_root: CFG[200..232].try_into().unwrap(),
        protocol_contracts_hash: CFG[232..264].try_into().unwrap(),
        proof_submission_epochs: u(264),
        rollup: CFG[272..292].try_into().unwrap(),
        seed: CFG[292..324].try_into().unwrap(),
        committee: CFG[325..325 + 20 * (CFG[324] as usize).min(16)].chunks_exact(20).map(|a| a.try_into().unwrap()).collect(),
        jam_era: match CFG.len() {
            n if n >= 653 && u(645) != 0 => u(645),
            _ => JAM_COMMON_ERA,
        },
    }
}

// Refused in refine: the first byte after 0.
const BAD_PAYLOAD: u8 = 1;
const BAD_HEADER: u8 = 2;
const BAD_BLOBS: u8 = 3;
const BAD_SIGNATURES: u8 = 6;
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
// The epoch path's verdicts.
const BAD_PROOF: u8 = 4;
const BAD_BLOB_OPENING: u8 = 5;
const WRONG_VK: u8 = 30;
const BAD_RANGE: u8 = 31;
const NOT_ONE_EPOCH: u8 = 32;
const NOT_EPOCH_START: u8 = 33;
const NOT_ON_PROVEN: u8 = 34;
const PAST_DEADLINE: u8 = 35;
const WRONG_PUBLIC_INPUT: u8 = 36;
const ALREADY_PROVEN: u8 = 37;

struct Attestations<'a> {
    modifier: [u8; 32],
    indices: &'a [u8],
    sigs: &'a [u8],
    signers: Vec<[u8; 20]>,
    proposer: signatures::Sig,
}

fn parse_attestations(p: &[u8]) -> Option<Attestations<'_>> {
    let modifier: [u8; 32] = p.get(..32)?.try_into().ok()?;
    let mut at = 32;
    let mut take = |n: usize| -> Option<&[u8]> { let b = p.get(at..at + n)?; at += n; Some(b) };
    let len = |b: &[u8]| u16::from_le_bytes([b[0], b[1]]) as usize;
    let il = len(take(2)?);
    let indices = take(il)?;
    let sl = len(take(2)?);
    let sigs = take(sl)?;
    let n = len(take(2)?);
    let signers = take(20 * n)?.chunks_exact(20).map(|a| a.try_into().unwrap()).collect();
    let proposer = take(65)?.try_into().ok()?;
    (at == p.len()).then_some(Attestations { modifier, indices, sigs, signers, proposer })
}

fn checkpoint(p: &[u8]) -> Result<Vec<u8>, u8> {
    let c = cfg();
    let header = Header::decode(p.get(..HEADER_LEN).ok_or(BAD_PAYLOAD)?).ok_or(BAD_PAYLOAD)?;
    let archive: [u8; 32] = p.get(HEADER_LEN..HEADER_LEN + 32).ok_or(BAD_PAYLOAD)?.try_into().unwrap();
    let hint: [u8; 8] = p.get(HEADER_LEN + 32..HEADER_LEN + 40).ok_or(BAD_PAYLOAD)?.try_into().unwrap();
    let rest = p.get(HEADER_LEN + 40..).ok_or(BAD_PAYLOAD)?;
    let n = *rest.first().ok_or(BAD_PAYLOAD)? as usize;
    let blobs = rest.get(..1 + 96 * n).ok_or(BAD_PAYLOAD)?;
    if n == 0 {
        return Err(BAD_PAYLOAD);
    }
    let items: Vec<&[u8]> = blobs[1..].chunks_exact(96).collect();
    let att = parse_attestations(&rest[1 + 96 * n..]).ok_or(BAD_PAYLOAD)?;
    let mut prefixed = alloc::vec![n as u8];
    items.iter().for_each(|it| prefixed.extend_from_slice(&it[..48]));
    let commitments = parse_blob_commitments(&prefixed).ok_or(BAD_PAYLOAD)?;
    check_header(&header, &aztec_l1::blobs_hash(&commitments), &c.mana_limit).map_err(|_| BAD_HEADER)?;
    let slot = header.slot().ok_or(BAD_HEADER)?;
    let digest = signatures::payload_digest(&archive, &att.modifier, &header.hash(), &c.chain_id, &c.rollup);
    signatures::verify_proposer(&c.committee, &c.seed, slot / c.epoch_slots, slot, &digest, att.indices, att.sigs, &att.signers, &att.proposer, &c.chain_id, &c.rollup)
        .map_err(|_| BAD_SIGNATURES)?;
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
    out.extend_from_slice(&header.coinbase);
    out.extend_from_slice(&header.accumulated_fees);
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

fn unix_time(era: u64, timeslot: u32) -> u64 {
    era + timeslot as u64 * JAM_SLOT_SECONDS
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
    let (coinbase, fees): ([u8; 20], [u8; 32]) = (o[200..220].try_into().unwrap(), w(220));
    let n = o[252] as usize;
    let commitments: Vec<[u8; 48]> = o[253..253 + 48 * n].chunks_exact(48).map(|x| x.try_into().unwrap()).collect();
    if last_archive != t.archive {
        return WRONG_PARENT;
    }
    if slot <= t.slot {
        return SLOT_NOT_AFTER_TIP;
    }
    let now = unix_time(c.jam_era, timeslot);
    // One slot of tolerance: on Ethereum a propose is timed by the L1 block that includes it,
    // which is at or after the slot's start. Here the clock is the JAM timeslot of the block
    // that accumulates the package, which lags the proposer's wall clock by guaranteeing and
    // finality (about 12 to 18 s on lasair), so a checkpoint proposed at its slot's start can
    // arrive with the service's clock still in the previous slot.
    let current_slot = (now + c.slot_seconds).saturating_sub(c.genesis_time) / c.slot_seconds;
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
    // [inbox messages consumed u64][bucket u64][coinbase 20][accumulated fees 32]
    let mut log = Vec::with_capacity(236);
    log.extend_from_slice(&header_hash);
    log.extend_from_slice(&archive);
    log.extend_from_slice(&slot.to_le_bytes());
    log.extend_from_slice(&out_hash);
    log.extend_from_slice(&inbox);
    log.extend_from_slice(&bch);
    log.extend_from_slice(&consumed.to_le_bytes());
    log.extend_from_slice(&hint.to_le_bytes());
    log.extend_from_slice(&coinbase);
    log.extend_from_slice(&fees);
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

fn epoch_proof(p: &[u8]) -> Result<Vec<u8>, u8> {
    let le = |at: usize, n: usize| -> Result<u64, u8> {
        let b = p.get(at..at + n).ok_or(BAD_PAYLOAD)?;
        Ok(b.iter().rev().fold(0u64, |a, &x| (a << 8) | x as u64))
    };
    let (start, end) = (le(0, 8)?, le(8, 8)?);
    let vk_len = le(16, 2)? as usize;
    let vk = p.get(18..18 + vk_len).ok_or(BAD_PAYLOAD)?;
    let mut at = 18 + vk_len;
    let pubs: Vec<[u8; 32]> = p
        .get(at..at + 32 * ROOT_ROLLUP_PUBLIC_INPUTS)
        .ok_or(BAD_PAYLOAD)?
        .chunks_exact(32)
        .map(|c| c.try_into().unwrap())
        .collect();
    at += 32 * ROOT_ROLLUP_PUBLIC_INPUTS;
    let proof_len = le(at, 4)? as usize;
    let proof = p.get(at + 4..at + 4 + proof_len).ok_or(BAD_PAYLOAD)?;
    let batched: [u8; 192] = p.get(at + 4 + proof_len..).ok_or(BAD_PAYLOAD)?.try_into().map_err(|_| BAD_PAYLOAD)?;
    // the curve host calls 101 and 102 where the net has them (#58), else field-wise curve hooks: the
    // same arkworks arithmetic, 14% cheaper than the default `()` (#48)
    verify::<curve_hooks::HostCallHooks>(vk, &ProofType::Plain(proof.into()), &pubs).map_err(|_| BAD_PROOF)?;
    if kzg_verify::point_evaluation(&batched) != Ok(true) || blob_public_inputs(&batched)[..] != pubs[epoch_pi::BLOB_Z..] {
        return Err(BAD_BLOB_OPENING);
    }
    let mut out = Vec::with_capacity(48 + 32 * epoch_pi::BLOB_Z);
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&end.to_le_bytes());
    out.extend_from_slice(&sha256_to_field(vk));
    pubs[..epoch_pi::BLOB_Z].iter().for_each(|x| out.extend_from_slice(x));
    Ok(out)
}

/// A stored checkpoint log, or the genesis values for checkpoint 0.
struct Log {
    header_hash: [u8; 32],
    archive: [u8; 32],
    slot: u64,
    out_hash: [u8; 32],
    inbox: [u8; 32],
    blob_commitments_hash: [u8; 32],
    coinbase: [u8; 20],
    fees: [u8; 32],
}

fn log(n: u64, c: &Cfg) -> Option<Log> {
    if n == 0 {
        return Some(Log { header_hash: [0; 32], archive: c.genesis_archive, slot: 0, out_hash: [0; 32], inbox: [0; 32], blob_commitments_hash: [0; 32], coinbase: [0; 20], fees: [0; 32] });
    }
    let l = get_storage(&key(b"cp:", n)).filter(|l| l.len() == 236)?;
    let w = |at: usize| -> [u8; 32] { l[at..at + 32].try_into().unwrap() };
    Some(Log {
        header_hash: w(0),
        archive: w(32),
        slot: u64::from_le_bytes(l[64..72].try_into().unwrap()),
        out_hash: w(72),
        inbox: w(104),
        blob_commitments_hash: w(136),
        coinbase: l[184..204].try_into().unwrap(),
        fees: w(204),
    })
}

fn proven() -> u64 {
    get_storage(b"tip:proven").map_or(0, |v| u64::from_le_bytes(v[..8].try_into().unwrap()))
}

fn accept_epoch(o: &[u8], timeslot: u32) -> u8 {
    let c = cfg();
    let u = |at: usize| u64::from_le_bytes(o[at..at + 8].try_into().unwrap());
    let (start, end) = (u(0), u(8));
    if o[16..48] != c.vk_hash {
        return WRONG_VK;
    }
    let pi = |i: usize| -> [u8; 32] { o[48 + 32 * i..80 + 32 * i].try_into().unwrap() };
    let pending = tip(&c).number;
    if start == 0 || end < start || end > pending || (end - start + 1) as usize > MAX_CHECKPOINTS_PER_EPOCH {
        return BAD_RANGE;
    }
    let logs: Vec<Log> = match (start - 1..=end).map(|n| log(n, &c)).collect::<Option<_>>() {
        Some(l) => l,
        None => return BAD_RANGE,
    };
    let (parent, first, last) = (&logs[0], &logs[1], &logs[logs.len() - 1]);
    let epoch = |slot: u64| slot / c.epoch_slots;
    if epoch(first.slot) != epoch(last.slot) {
        return NOT_ONE_EPOCH;
    }
    if !(start == 1 || epoch(parent.slot) < epoch(first.slot)) {
        return NOT_EPOCH_START;
    }
    let proven_before = proven();
    if start - 1 > proven_before {
        return NOT_ON_PROVEN;
    }
    let current_epoch = epoch(unix_time(c.jam_era, timeslot).saturating_sub(c.genesis_time) / c.slot_seconds);
    if current_epoch >= epoch(first.slot) + c.proof_submission_epochs + 1 {
        return PAST_DEADLINE;
    }
    // `computeEpochProofPublicInputs`, compared
    let mut want = alloc::vec![[0u8; 32]; epoch_pi::BLOB_Z];
    want[epoch_pi::PREVIOUS_ARCHIVE] = parent.archive;
    want[epoch_pi::END_ARCHIVE] = last.archive;
    want[epoch_pi::OUT_HASH] = last.out_hash;
    want[epoch_pi::PREVIOUS_INBOX] = parent.inbox;
    want[epoch_pi::END_INBOX] = last.inbox;
    for (i, l) in logs[1..].iter().enumerate() {
        want[epoch_pi::HEADER_HASHES + i] = l.header_hash;
        want[epoch_pi::FEES + 2 * i] = address_to_field(&l.coinbase);
        want[epoch_pi::FEES + 2 * i + 1] = l.fees;
    }
    want[epoch_pi::CHAIN_ID] = c.chain_id;
    want[epoch_pi::VERSION] = c.version;
    want[epoch_pi::VK_TREE_ROOT] = c.vk_tree_root;
    want[epoch_pi::PROTOCOL_CONTRACTS_HASH] = c.protocol_contracts_hash;
    want[epoch_pi::PROVER_ID] = pi(epoch_pi::PROVER_ID); // the prover names itself
    want[epoch_pi::BLOB_COMMITMENTS_HASH] = last.blob_commitments_hash;
    if (0..epoch_pi::BLOB_Z).any(|i| pi(i) != want[i]) {
        return WRONG_PUBLIC_INPUT;
    }
    if end <= proven_before {
        return ALREADY_PROVEN;
    }
    set_storage(b"tip:proven", &[&end.to_le_bytes()[..], &last.archive].concat()).ok();
    set_storage(&key(b"prover:", end), &pi(epoch_pi::PROVER_ID)).ok();
    if last.out_hash != EMPTY_EPOCH_OUT_HASH {
        set_storage(&key(b"outbox:", epoch(last.slot)), &[&last.out_hash[..], &(end - start + 1).to_le_bytes()].concat()).ok();
    }
    ACCEPTED
}

impl Service for Settlement {
    fn refine(_core: CoreIndex, item: usize, _service: ServiceId, payload: WorkPayload, _pkg: WorkPackageHash) -> WorkOutput {
        let p = payload.take();
        let tag = p.get(1).copied().unwrap_or(0);
        let out = match (p.first(), p.get(2..)) {
            (Some(1), Some(rest)) => checkpoint(rest),
            (Some(2), Some(rest)) => epoch_proof(rest),
            _ => Err(BAD_PAYLOAD),
        };
        match out {
            Ok(o) => [&[p[0], tag][..], &o].concat().into(),
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
                    if !absorb(t.source, &t.memo.0[..], unix_time(c.jam_era, timeslot), &c) {
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
                Some(1) if o.len() >= 255 && o.len() == 255 + 48 * o[254] as usize => accept(&o[2..], timeslot),
                Some(2) if o.len() == 2 + 16 + 32 + 32 * epoch_pi::BLOB_Z => accept_epoch(&o[2..], timeslot),
                _ => o.get(2).copied().unwrap_or(0),
            };
            set_storage(&[b'v', b':', tag], &[verdict]).ok();
        }
        None
    }
}
