//! Aztec v6's L1 encodings and the checks that need no state, ported from aztec-packages v6.0.0
//! (0219f11) `l1-contracts/src/core/libraries`: `crypto/Hash.sol`, `rollup/ProposedHeaderLib.sol`,
//! `rollup/BlobLib.sol`, and the stateless half of `rollup/ProposeLib.sol`'s `validateHeader`.
//! The settlement service's refine and accumulate use it.
#![no_std]

extern crate alloc;

pub mod signatures;

use alloc::vec::Vec;
use sha2::{Digest, Sha256};

/// `Hash.sha256ToField`: sha256, the top byte dropped, so the value is below the BN254 modulus.
pub fn sha256_to_field(data: &[u8]) -> [u8; 32] {
    let h = Sha256::digest(data);
    let mut o = [0u8; 32];
    o[1..].copy_from_slice(&h[..31]);
    o
}

/// BN254's scalar field modulus, big-endian (`FieldLib.requireValidFieldElement`).
const FR_MODULUS: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93, 0xf0, 0x00, 0x00, 0x01,
];

pub fn is_field(x: &[u8; 32]) -> bool {
    x < &FR_MODULUS
}

/// `ProposedHeader`, in `ProposedHeaderLib.hash`'s packed encoding: 348 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub last_archive_root: [u8; 32],
    pub block_headers_hash: [u8; 32],
    pub blobs_hash: [u8; 32],
    pub inbox_rolling_hash: [u8; 32],
    pub out_hash: [u8; 32],
    pub slot_number: [u8; 32],
    pub timestamp: u64,
    pub coinbase: [u8; 20],
    pub fee_recipient: [u8; 32],
    pub fee_per_da_gas: u128,
    pub fee_per_l2_gas: u128,
    pub total_mana_used: [u8; 32],
    pub accumulated_fees: [u8; 32],
}

pub const HEADER_LEN: usize = 348;

impl Header {
    pub fn decode(b: &[u8]) -> Option<Header> {
        if b.len() != HEADER_LEN {
            return None;
        }
        let w = |at: usize| -> [u8; 32] { b[at..at + 32].try_into().unwrap() };
        Some(Header {
            last_archive_root: w(0),
            block_headers_hash: w(32),
            blobs_hash: w(64),
            inbox_rolling_hash: w(96),
            out_hash: w(128),
            slot_number: w(160),
            timestamp: u64::from_be_bytes(b[192..200].try_into().unwrap()),
            coinbase: b[200..220].try_into().unwrap(),
            fee_recipient: w(220),
            fee_per_da_gas: u128::from_be_bytes(b[252..268].try_into().unwrap()),
            fee_per_l2_gas: u128::from_be_bytes(b[268..284].try_into().unwrap()),
            total_mana_used: w(284),
            accumulated_fees: w(316),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(HEADER_LEN);
        for w in [&self.last_archive_root, &self.block_headers_hash, &self.blobs_hash, &self.inbox_rolling_hash, &self.out_hash, &self.slot_number] {
            o.extend_from_slice(w);
        }
        o.extend_from_slice(&self.timestamp.to_be_bytes());
        o.extend_from_slice(&self.coinbase);
        o.extend_from_slice(&self.fee_recipient);
        o.extend_from_slice(&self.fee_per_da_gas.to_be_bytes());
        o.extend_from_slice(&self.fee_per_l2_gas.to_be_bytes());
        o.extend_from_slice(&self.total_mana_used);
        o.extend_from_slice(&self.accumulated_fees);
        o
    }

    /// `ProposedHeaderLib.hash`.
    pub fn hash(&self) -> [u8; 32] {
        sha256_to_field(&self.encode())
    }

    /// The slot as a u64, if it fits (Aztec's slots do).
    pub fn slot(&self) -> Option<u64> {
        self.slot_number[..24].iter().all(|&x| x == 0).then(|| u64::from_be_bytes(self.slot_number[24..].try_into().unwrap()))
    }
}

/// Why a header is refused, in `validateHeader`'s order.
#[derive(Debug, PartialEq, Eq)]
pub enum HeaderError {
    NotAField,
    InvalidCoinbase,
    ManaLimitExceeded,
    UnavailableTxs,
    NonZeroDaFee,
}

/// The checks `validateHeader` makes from the header alone. The rest (the parent archive, the slot
/// against the clock and the last slot, the mana minimum fee) need state, so accumulate makes them.
pub fn check_header(h: &Header, blobs_hash: &[u8; 32], mana_limit: &[u8; 32]) -> Result<(), HeaderError> {
    for f in [&h.block_headers_hash, &h.out_hash, &h.fee_recipient, &h.accumulated_fees] {
        if !is_field(f) {
            return Err(HeaderError::NotAField);
        }
    }
    if h.coinbase == [0u8; 20] {
        return Err(HeaderError::InvalidCoinbase);
    }
    if &h.total_mana_used > mana_limit {
        return Err(HeaderError::ManaLimitExceeded);
    }
    if &h.blobs_hash != blobs_hash {
        return Err(HeaderError::UnavailableTxs);
    }
    if h.fee_per_da_gas != 0 {
        return Err(HeaderError::NonZeroDaFee);
    }
    Ok(())
}

/// `BlobLib.calculateBlobHash`: EIP-4844's versioned hash, 0x01 ‖ sha256(C)[1..].
pub fn blob_hash(commitment: &[u8; 48]) -> [u8; 32] {
    let mut h: [u8; 32] = Sha256::digest(commitment).into();
    h[0] = 0x01;
    h
}

/// `BlobLib.validateBlobs`'s input: [n u8][n × 48-byte commitment].
pub fn parse_blob_commitments(input: &[u8]) -> Option<Vec<[u8; 48]>> {
    let n = *input.first()? as usize;
    if n == 0 || input.len() < 1 + 48 * n {
        return None;
    }
    Some(input[1..1 + 48 * n].chunks_exact(48).map(|c| c.try_into().unwrap()).collect())
}

/// The header's `blobsHash`: sha256ToField of the checkpoint's blob hashes.
pub fn blobs_hash(commitments: &[[u8; 48]]) -> [u8; 32] {
    let mut b = Vec::with_capacity(32 * commitments.len());
    for c in commitments {
        b.extend_from_slice(&blob_hash(c));
    }
    sha256_to_field(&b)
}

/// `BlobLib.calculateBlobCommitmentsHash`: the epoch's running hash of blob commitments, each
/// with a flag byte, 1 for a checkpoint's first blob.
pub fn blob_commitments_hash(previous: &[u8; 32], commitments: &[[u8; 48]], first_of_epoch: bool) -> [u8; 32] {
    let mut acc = *previous;
    for (i, c) in commitments.iter().enumerate() {
        let mut b = Vec::with_capacity(81);
        if !(first_of_epoch && i == 0) {
            b.extend_from_slice(&acc);
        }
        b.push((i == 0) as u8);
        b.extend_from_slice(c);
        acc = sha256_to_field(&b);
    }
    acc
}

/// `Constants.DOM_SEP__INBOX_ROLLING_HASH`.
pub const DOM_SEP_INBOX_ROLLING_HASH: u32 = 3_737_216_265;

/// An L1→L2 message leaf (`Hash.sha256ToField(L1ToL2Msg)`, `Inbox.sendL2Message`): sha256ToField of
/// abi.encode(sender, chain id, recipient, version, content, secret hash, index), seven words.
/// On JAM the sender is the sending service's id, as a 20-byte address.
pub fn l1_to_l2_leaf(sender: u32, chain_id: &[u8; 32], recipient: &[u8; 32], version: &[u8; 32], content: &[u8; 32], secret_hash: &[u8; 32], index: u64) -> [u8; 32] {
    let mut b = [0u8; 224];
    b[28..32].copy_from_slice(&sender.to_be_bytes());
    b[32..64].copy_from_slice(chain_id);
    b[64..96].copy_from_slice(recipient);
    b[96..128].copy_from_slice(version);
    b[128..160].copy_from_slice(content);
    b[160..192].copy_from_slice(secret_hash);
    b[216..224].copy_from_slice(&index.to_be_bytes());
    sha256_to_field(&b)
}

/// `Hash.accumulateInboxRollingHash`: sha256ToField(u32 domain separator ‖ previous ‖ leaf).
pub fn inbox_rolling_hash(previous: &[u8; 32], leaf: &[u8; 32]) -> [u8; 32] {
    let mut b = [0u8; 68];
    b[..4].copy_from_slice(&DOM_SEP_INBOX_ROLLING_HASH.to_be_bytes());
    b[4..36].copy_from_slice(previous);
    b[36..].copy_from_slice(leaf);
    sha256_to_field(&b)
}

/// `Constants.ROOT_ROLLUP_PUBLIC_INPUTS_LENGTH` and `MAX_CHECKPOINTS_PER_EPOCH`.
pub const ROOT_ROLLUP_PUBLIC_INPUTS: usize = 113;
pub const MAX_CHECKPOINTS_PER_EPOCH: usize = 32;
/// Where `EpochProofLib.computeEpochProofPublicInputs` puts each value.
pub mod epoch_pi {
    pub const PREVIOUS_ARCHIVE: usize = 0;
    pub const END_ARCHIVE: usize = 1;
    pub const OUT_HASH: usize = 2;
    pub const PREVIOUS_INBOX: usize = 3;
    pub const END_INBOX: usize = 4;
    pub const HEADER_HASHES: usize = 5;
    pub const FEES: usize = 5 + super::MAX_CHECKPOINTS_PER_EPOCH;
    pub const CHAIN_ID: usize = FEES + 2 * super::MAX_CHECKPOINTS_PER_EPOCH;
    pub const VERSION: usize = CHAIN_ID + 1;
    pub const VK_TREE_ROOT: usize = CHAIN_ID + 2;
    pub const PROTOCOL_CONTRACTS_HASH: usize = CHAIN_ID + 3;
    pub const PROVER_ID: usize = CHAIN_ID + 4;
    pub const BLOB_COMMITMENTS_HASH: usize = CHAIN_ID + 5;
    pub const BLOB_Z: usize = CHAIN_ID + 6;
}

/// `Constants.EMPTY_EPOCH_OUT_HASH`: an epoch with no L2→L1 messages.
pub const EMPTY_EPOCH_OUT_HASH: [u8; 32] = [
    0x00, 0xc9, 0x5e, 0x0c, 0xeb, 0x41, 0x95, 0x10, 0x39, 0xe1, 0x59, 0x27, 0x45, 0xec, 0x2f, 0xae,
    0xa9, 0x86, 0x6f, 0x6e, 0xaf, 0x01, 0xbf, 0x18, 0x9a, 0x44, 0x63, 0xb4, 0x14, 0x3a, 0xf0, 0x93,
];

/// `EpochProofLib.addressToField`.
pub fn address_to_field(a: &[u8; 20]) -> [u8; 32] {
    let mut o = [0u8; 32];
    o[12..].copy_from_slice(a);
    o
}

/// The last six public inputs, from the batched blob opening (`EpochProofLib`): z; y as three
/// limbs (`bytes32ToBigNum`: the low 120 bits, the next 120, the top 16); the commitment as its
/// first 31 bytes and its last 17. `batched` is `BlobLib.validateBatchedBlob`'s 192-byte input:
/// [versioned hash 32][z 32][y 32][C 48][proof 48].
pub fn blob_public_inputs(batched: &[u8; 192]) -> [[u8; 32]; 6] {
    let right = |src: &[u8]| -> [u8; 32] { let mut o = [0u8; 32]; o[32 - src.len()..].copy_from_slice(src); o };
    let (z, y, c) = (&batched[32..64], &batched[64..96], &batched[96..144]);
    [right(z), right(&y[17..32]), right(&y[2..17]), right(&y[..2]), right(&c[..31]), right(&c[31..48])]
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::{format, fs, string::String};

    fn unhex(s: &str) -> Vec<u8> {
        let s = s.trim_start_matches("0x");
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
    fn w32(s: &str) -> [u8; 32] { unhex(s).try_into().unwrap() }
    fn word(n: u64) -> [u8; 32] { let mut o = [0u8; 32]; o[24..].copy_from_slice(&n.to_be_bytes()); o }

    fn fixture(name: &str) -> serde_json::Value { fixture_at(&format!("v6/{name}")) }
    fn fixture_at(path: &str) -> serde_json::Value {
        let p = format!("{}/fixtures/{path}.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
    }
    fn word_of(v: &serde_json::Value) -> [u8; 32] {
        match v {
            serde_json::Value::Number(n) => word(n.as_u64().unwrap()),
            serde_json::Value::String(s) => { let b = unhex(s); let mut o = [0u8; 32]; o[32 - b.len()..].copy_from_slice(&b); o }
            _ => panic!(),
        }
    }

    #[test]
    fn inbox_leaves_and_rolling_hash_match_aztecs_code() {
        let f = fixture_at("generated/inbox");
        let (spec, want) = (&f["spec"], &f["expected"]);
        let (chain, version) = (word_of(&spec["chainId"]), word_of(&spec["version"]));
        let mut leaves = Vec::new();
        for (i, m) in spec["messages"].as_array().unwrap().iter().enumerate() {
            let leaf = l1_to_l2_leaf(m["sender"].as_u64().unwrap() as u32, &chain, &word_of(&m["recipient"]), &version, &word_of(&m["content"]), &word_of(&m["secretHash"]), i as u64);
            assert_eq!(leaf, w32(want["leaves"][i].as_str().unwrap()), "leaf {i}");
            leaves.push(leaf);
        }
        let (mut rolling, mut at) = ([0u8; 32], 0);
        for (b, n) in spec["buckets"].as_array().unwrap().iter().enumerate() {
            for leaf in &leaves[at..at + n.as_u64().unwrap() as usize] { rolling = inbox_rolling_hash(&rolling, leaf); }
            at += n.as_u64().unwrap() as usize;
            assert_eq!(rolling, w32(want["buckets"][b]["rollingHash"].as_str().unwrap()), "bucket {b}");
        }
    }


    fn header(c: &serde_json::Value) -> Header {
        let h = &c["header"];
        let s = |k: &str| -> String { h[k].as_str().unwrap().into() };
        let n = |k: &str| -> u64 { h[k].as_u64().unwrap() };
        Header {
            last_archive_root: w32(&s("lastArchiveRoot")),
            block_headers_hash: w32(&s("blockHeadersHash")),
            blobs_hash: w32(&s("blobsHash")),
            inbox_rolling_hash: w32(&s("inboxRollingHash")),
            out_hash: w32(&s("outHash")),
            slot_number: word(n("slotNumber")),
            timestamp: n("timestamp"),
            coinbase: unhex(&s("coinbase")).try_into().unwrap(),
            fee_recipient: w32(&s("feeRecipient")),
            fee_per_da_gas: h["gasFees"]["feePerDaGas"].as_u64().unwrap() as u128,
            fee_per_l2_gas: h["gasFees"]["feePerL2Gas"].as_u64().unwrap() as u128,
            total_mana_used: word(n("totalManaUsed")),
            accumulated_fees: word(n("accumulatedFees")),
        }
    }

    const FIXTURES: [&str; 6] = ["empty_checkpoint_1", "empty_checkpoint_2", "single_tx_checkpoint_1", "single_tx_checkpoint_2", "mixed_checkpoint_1", "mixed_checkpoint_2"];



    #[test]
    fn signatures_match_aztecs_code() {
        use crate::signatures::*;
        let f = fixture_at("generated/signatures");
        let h = |k: &str| -> Vec<u8> { unhex(f[k].as_str().unwrap()) };
        let a20 = |s: &str| -> [u8; 20] { unhex(s).try_into().unwrap() };
        let chain = word(f["chainId"].as_u64().unwrap());
        let rollup = a20(f["rollup"].as_str().unwrap());
        let committee: Vec<[u8; 20]> = f["committee"].as_array().unwrap().iter().map(|x| a20(x.as_str().unwrap())).collect();
        let signers: Vec<[u8; 20]> = f["signers"].as_array().unwrap().iter().map(|x| a20(x.as_str().unwrap())).collect();
        let mut seed = [0u8; 32];
        seed[24..].copy_from_slice(&f["seed"].as_str().unwrap().parse::<u64>().unwrap().to_be_bytes());
        let modifier = word(f["feeAssetPriceModifier"].as_str().unwrap().parse::<u64>().unwrap());
        let digest = payload_digest(&w32(f["archive"].as_str().unwrap()), &modifier, &w32(f["headerHash"].as_str().unwrap()), &chain, &rollup);
        assert_eq!(digest, w32(f["payloadDigest"].as_str().unwrap()));
        let (idx, so) = (h("signatureIndices"), h("signaturesOrAddresses"));
        assert_eq!(attestations_and_signers_digest(&idx, &so, &signers, &chain, &rollup), w32(f["attestationsAndSignersDigest"].as_str().unwrap()));
        assert_eq!(committee_commitment(&committee), w32(f["committeeCommitment"].as_str().unwrap()));
        let (slot, epoch) = (f["slot"].as_u64().unwrap(), f["epoch"].as_u64().unwrap());
        assert_eq!(proposer_index(epoch, slot, &seed, 4), f["proposerIndex"].as_u64().unwrap() as usize);
        let s = &f["attestationsAndSignersSignature"];
        let mut psig = [0u8; 65];
        psig[0] = s["v"].as_u64().unwrap() as u8;
        psig[1..33].copy_from_slice(&unhex(s["r"].as_str().unwrap()));
        psig[33..].copy_from_slice(&unhex(s["s"].as_str().unwrap()));
        let ok = verify_proposer(&committee, &seed, epoch, slot, &digest, &idx, &so, &signers, &psig, &chain, &rollup);
        assert_eq!(ok, Ok(f["proposerIndex"].as_u64().unwrap() as usize));
        // refused: another digest, another committee, a corrupted proposer signature
        let mut other = digest; other[0] ^= 1;
        assert_eq!(verify_proposer(&committee, &seed, epoch, slot, &other, &idx, &so, &signers, &psig, &chain, &rollup), Err(SignatureError::BadProposerSignature));
        let mut swapped = committee.clone(); swapped.swap(1, 2);
        assert_eq!(verify_proposer(&swapped, &seed, epoch, slot, &digest, &idx, &so, &signers, &psig, &chain, &rollup), Err(SignatureError::WrongCommittee));
        let mut bad = psig; bad[40] ^= 1;
        assert!(verify_proposer(&committee, &seed, epoch, slot, &digest, &idx, &so, &signers, &bad, &chain, &rollup).is_err());
        assert_eq!(verify_proposer(&[], &seed, epoch, slot, &other, &idx, &so, &signers, &bad, &chain, &rollup), Ok(0));
    }

    #[test]
    fn epoch_layout_and_blob_limbs() {
        assert_eq!(epoch_pi::BLOB_Z + 6, ROOT_ROLLUP_PUBLIC_INPUTS);
        let mut b = [0u8; 192];
        for (i, x) in b.iter_mut().enumerate() { *x = i as u8; }
        let l = blob_public_inputs(&b);
        assert_eq!(&l[0][..], &b[32..64]);
        assert_eq!(&l[1][17..], &b[64 + 17..96]);
        assert!(l[1][..17].iter().all(|&x| x == 0));
        assert_eq!(&l[2][17..], &b[64 + 2..64 + 17]);
        assert_eq!(&l[3][30..], &b[64..66]);
        assert_eq!(&l[4][1..], &b[96..127]);
        assert_eq!(&l[5][15..], &b[127..144]);
    }

    #[test]
    fn header_hash_matches_every_fixture() {
        for f in FIXTURES {
            let c = &fixture(f)["checkpoint"];
            let h = header(c);
            assert_eq!(h.encode().len(), HEADER_LEN);
            assert_eq!(Header::decode(&h.encode()).unwrap(), h);
            assert_eq!(h.hash(), w32(c["headerHash"].as_str().unwrap()), "{f}");
        }
    }

    #[test]
    fn blobs_hash_matches_every_fixture() {
        for f in FIXTURES {
            let c = &fixture(f)["checkpoint"];
            let cs = parse_blob_commitments(&unhex(c["blobCommitments"].as_str().unwrap())).unwrap();
            assert_eq!(blobs_hash(&cs), header(c).blobs_hash, "{f}");
            // the batched opening's versioned hash is the batched commitment's
            let bb = unhex(c["batchedBlobInputs"].as_str().unwrap());
            assert_eq!(bb.len(), 192);
            assert_eq!(blob_hash(&bb[96..144].try_into().unwrap()), bb[..32], "{f}");
        }
    }

    #[test]
    fn every_fixture_passes_the_stateless_checks_and_a_tamper_fails() {
        let limit = [0xffu8; 32];
        for f in FIXTURES {
            let c = &fixture(f)["checkpoint"];
            let h = header(c);
            let cs = parse_blob_commitments(&unhex(c["blobCommitments"].as_str().unwrap())).unwrap();
            assert_eq!(check_header(&h, &blobs_hash(&cs), &limit), Ok(()), "{f}");
            let mut bad = h.clone();
            bad.coinbase = [0; 20];
            assert_eq!(check_header(&bad, &blobs_hash(&cs), &limit), Err(HeaderError::InvalidCoinbase));
            let mut bad = h.clone();
            bad.out_hash = FR_MODULUS;
            assert_eq!(check_header(&bad, &blobs_hash(&cs), &limit), Err(HeaderError::NotAField));
            assert_eq!(check_header(&h, &[0; 32], &limit), Err(HeaderError::UnavailableTxs));
            let mut bad = h.clone();
            bad.timestamp += 1;
            assert_ne!(bad.hash(), h.hash());
        }
    }

    #[test]
    fn the_second_checkpoint_builds_on_the_first() {
        for p in ["empty_checkpoint", "single_tx_checkpoint", "mixed_checkpoint"] {
            let (a, b) = (fixture(&format!("{p}_1")), fixture(&format!("{p}_2")));
            assert_eq!(header(&b["checkpoint"]).last_archive_root, w32(a["checkpoint"]["archive"].as_str().unwrap()), "{p}");
        }
    }
}
