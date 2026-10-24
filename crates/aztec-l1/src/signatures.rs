//! The committee's signatures on a proposal, as Aztec v6's Rollup checks them (#54):
//! `CoordinationSignatureLib` (EIP-712 in the "Aztec Rollup" domain, version 1),
//! `ProposeLib.digest`, `AttestationLib`, `ValidatorSelectionLib.verifyProposer` and
//! `SignatureLib.verify` (OpenZeppelin's ECDSA.recover: v 27 or 28, s in the lower half).
use alloc::vec::Vec;
use k256::ecdsa::{RecoveryId, Signature as K256Signature, VerifyingKey};
use sha3::{Digest, Keccak256};

pub fn keccak(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

fn word_u64(n: u64) -> [u8; 32] {
    let mut o = [0u8; 32];
    o[24..].copy_from_slice(&n.to_be_bytes());
    o
}

fn word_addr(a: &[u8; 20]) -> [u8; 32] {
    let mut o = [0u8; 32];
    o[12..].copy_from_slice(a);
    o
}

/// The EIP-712 hash of a struct in the Rollup's domain (`toTypedDataHash`).
fn typed(struct_hash: &[u8; 32], chain_id: &[u8; 32], rollup: &[u8; 20]) -> [u8; 32] {
    let domain = keccak(&[
        keccak(b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
        keccak(b"Aztec Rollup"),
        keccak(b"1"),
        *chain_id,
        word_addr(rollup),
    ]
    .concat());
    keccak(&[&[0x19u8, 0x01][..], &domain, struct_hash].concat())
}

/// `ProposeLib.digest`: what each committee member signs. `fee_asset_price_modifier` is the
/// int256 of the oracle input, as 32 bytes big-endian two's complement.
pub fn payload_digest(archive: &[u8; 32], fee_asset_price_modifier: &[u8; 32], header_hash: &[u8; 32], chain_id: &[u8; 32], rollup: &[u8; 20]) -> [u8; 32] {
    let payload = keccak(&[*archive, *fee_asset_price_modifier, *header_hash].concat());
    typed(&keccak(&[keccak(b"CheckpointAttestation(bytes32 payloadHash)"), payload].concat()), chain_id, rollup)
}

/// Solidity's encoding of `bytes`: length, then the data padded to whole words.
fn abi_bytes(b: &[u8]) -> Vec<u8> {
    let mut o = word_u64(b.len() as u64).to_vec();
    o.extend_from_slice(b);
    o.resize(32 + b.len().div_ceil(32) * 32, 0);
    o
}

/// `AttestationLib.getAttestationsAndSignersDigest`: what the proposer signs, over
/// abi.encode(CommitteeAttestations { signatureIndices, signaturesOrAddresses }, address[] signers).
pub fn attestations_and_signers_digest(indices: &[u8], sigs_or_addrs: &[u8], signers: &[[u8; 20]], chain_id: &[u8; 32], rollup: &[u8; 20]) -> [u8; 32] {
    let (a, b) = (abi_bytes(indices), abi_bytes(sigs_or_addrs));
    let tuple = [&word_u64(64)[..], &word_u64(64 + a.len() as u64), &a, &b].concat();
    let mut array = word_u64(signers.len() as u64).to_vec();
    signers.iter().for_each(|s| array.extend_from_slice(&word_addr(s)));
    let encoded = [&word_u64(64)[..], &word_u64(64 + tuple.len() as u64), &tuple, &array].concat();
    typed(&keccak(&[keccak(b"AttestationsAndSigners(bytes32 payloadHash)"), keccak(&encoded)].concat()), chain_id, rollup)
}

/// `ValidatorSelectionLib.computeCommitteeCommitment`: keccak256(abi.encode(address[])).
pub fn committee_commitment(committee: &[[u8; 20]]) -> [u8; 32] {
    let mut e = [word_u64(32), word_u64(committee.len() as u64)].concat();
    committee.iter().for_each(|a| e.extend_from_slice(&word_addr(a)));
    keccak(&e)
}

/// `ValidatorSelectionLib.computeProposerIndex`.
pub fn proposer_index(epoch: u64, slot: u64, seed: &[u8; 32], size: usize) -> usize {
    let h = keccak(&[word_u64(epoch), word_u64(slot), *seed].concat());
    // h mod size, over the 256-bit big-endian value
    (h.iter().fold(0u128, |acc, &x| ((acc << 8) | x as u128) % size as u128)) as usize
}

/// A signature from the packed attestations: v (1), r (32), s (32).
pub type Sig = [u8; 65];

/// `AttestationLib.reconstructCommitteeFromSigners`: each member's address, and its signature
/// where it signed. None where the packing or the signer count disagrees with the length.
pub fn reconstruct_committee(indices: &[u8], sigs_or_addrs: &[u8], signers: &[[u8; 20]], length: usize) -> Option<(Vec<[u8; 20]>, Vec<Option<Sig>>)> {
    if indices.len() != length.div_ceil(8) {
        return None;
    }
    let (mut addrs, mut sigs, mut at, mut next_signer) = (Vec::new(), Vec::new(), 0usize, 0usize);
    for i in 0..length {
        if (indices[i / 8] >> (7 - i % 8)) & 1 == 1 {
            sigs.push(Some(sigs_or_addrs.get(at..at + 65)?.try_into().ok()?));
            addrs.push(*signers.get(next_signer)?);
            next_signer += 1;
            at += 65;
        } else {
            sigs.push(None);
            addrs.push(sigs_or_addrs.get(at..at + 20)?.try_into().ok()?);
            at += 20;
        }
    }
    (at == sigs_or_addrs.len() && next_signer == signers.len()).then_some((addrs, sigs))
}

/// OpenZeppelin's `ECDSA.recover`: v 27 or 28, r and s nonzero, s in the lower half of the order.
pub fn recover(digest: &[u8; 32], sig: &Sig) -> Option<[u8; 20]> {
    let (v, rs) = (sig[0], &sig[1..]);
    if v != 27 && v != 28 {
        return None;
    }
    let s = K256Signature::from_slice(rs).ok()?;
    if s.normalize_s().is_some() {
        return None; // s is in the upper half: OpenZeppelin rejects it
    }
    let key = VerifyingKey::recover_from_prehash(digest, &s, RecoveryId::from_byte(v - 27)?).ok()?;
    let point = key.to_encoded_point(false);
    let h = keccak(&point.as_bytes()[1..]);
    Some(h[12..].try_into().unwrap())
}

#[derive(Debug, PartialEq, Eq)]
pub enum SignatureError {
    BadPacking,
    WrongCommittee,
    MissingProposerSignature,
    BadProposerSignature,
    BadAttestationsSignature,
}

/// `ValidatorSelectionLib.verifyProposer`, for a committee fixed in configuration: the
/// attestations reconstruct it, the slot's proposer signed the payload digest, and signed the
/// attestations and signers. An empty committee skips the check, as the Rollup does.
#[allow(clippy::too_many_arguments)]
pub fn verify_proposer(committee: &[[u8; 20]], seed: &[u8; 32], epoch: u64, slot: u64, digest: &[u8; 32], indices: &[u8], sigs_or_addrs: &[u8], signers: &[[u8; 20]], proposer_sig: &Sig, chain_id: &[u8; 32], rollup: &[u8; 20]) -> Result<usize, SignatureError> {
    if committee.is_empty() {
        return Ok(0);
    }
    let (members, sigs) = reconstruct_committee(indices, sigs_or_addrs, signers, committee.len()).ok_or(SignatureError::BadPacking)?;
    if committee_commitment(&members) != committee_commitment(committee) {
        return Err(SignatureError::WrongCommittee);
    }
    let p = proposer_index(epoch, slot, seed, committee.len());
    let sig = sigs[p].ok_or(SignatureError::MissingProposerSignature)?;
    if recover(digest, &sig) != Some(members[p]) {
        return Err(SignatureError::BadProposerSignature);
    }
    let aas = attestations_and_signers_digest(indices, sigs_or_addrs, signers, chain_id, rollup);
    if recover(&aas, proposer_sig) != Some(members[p]) {
        return Err(SignatureError::BadAttestationsSignature);
    }
    Ok(p)
}
