// #54: a signed checkpoint proposal, by Aztec v6's released code, for the settlement service's
// signature checks. A committee of four test keys; the slot's proposer is index
// keccak256(abi.encode(epoch, slot, seed)) % 4 (ValidatorSelectionLib.computeProposerIndex).
// Three members sign the payload digest (computeCheckpointPayloadDigest); the fourth is listed
// by address. The proposer also signs the attestations-and-signers payload
// (CommitteeAttestationsAndSigners, getHashedSignaturePayloadTypedData, as the Rollup's
// AttestationLib.getAttestationsAndSignersDigest hashes it).
//   node sign-vectors.mjs <out.json> [chain id] [rollup address] [slot] [epoch length]
import { computeCheckpointPayloadDigest } from '@aztec-labs/stdlib/checkpoint';
import { CheckpointHeader } from '@aztec-labs/stdlib/rollup';
import { CommitteeAttestation, CommitteeAttestationsAndSigners } from '@aztec-labs/stdlib/block';
import { getHashedSignaturePayloadTypedData } from '@aztec-labs/stdlib/p2p';
import { Secp256k1Signer } from '@aztec-labs/foundation/crypto/secp256k1-signer';
import { Buffer32 } from '@aztec-labs/foundation/buffer';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { EthAddress } from '@aztec-labs/foundation/eth-address';
import { SlotNumber } from '@aztec-labs/foundation/branded-types';
import { encodeAbiParameters, keccak256 } from 'viem';
import { writeFileSync } from 'fs';

const [out, chainId = '31337', rollup = '0x00000000000000000000000000000000a11ce002', slotArg = '7', epochLen = '4'] = process.argv.slice(2);
const ctx = { chainId: Number(chainId), rollupAddress: EthAddress.fromString(rollup) };
const slot = BigInt(slotArg), epoch = slot / BigInt(epochLen), seed = 12345n;
const keys = [1, 2, 3, 4].map((i) => new Secp256k1Signer(Buffer32.fromBigInt(0x1000n + BigInt(i))));
const committee = keys.map((k) => k.address);
const proposerIndex = Number(BigInt(keccak256(encodeAbiParameters([{ type: 'uint256' }, { type: 'uint256' }, { type: 'uint256' }], [epoch, slot, seed]))) % 4n);
const header = CheckpointHeader.random({ slotNumber: SlotNumber(Number(slot)) });
const archive = Fr.random();
const modifier = 7n;
const digest = computeCheckpointPayloadDigest({ header, archiveRoot: archive, feeAssetPriceModifier: modifier, signatureContext: ctx });
const absent = (proposerIndex + 1) % 4;               // listed by address, not signing
const atts = keys.map((k, i) => (i === absent ? CommitteeAttestation.fromAddress(k.address) : CommitteeAttestation.fromAddressAndSignature(k.address, k.sign(Buffer32.fromString(digest.toString())))));
const aas = new CommitteeAttestationsAndSigners(atts, ctx);
const packed = aas.getPackedAttestations();
const signers = aas.getSigners().map((s) => s.toString());
const aasDigest = getHashedSignaturePayloadTypedData(aas);   // the EIP-712 hash the Rollup checks
const aasSig = keys[proposerIndex].sign(aasDigest);
const sig = (s) => ({ v: s.v, r: s.r.toString(), s: s.s.toString() });
writeFileSync(out, JSON.stringify({
  chainId: ctx.chainId, rollup, slot: Number(slot), epoch: Number(epoch), seed: seed.toString(), committee: committee.map((a) => a.toString()), proposerIndex,
  archive: archive.toString(), headerHash: header.hash().toString(), feeAssetPriceModifier: modifier.toString(), payloadDigest: digest.toString(),
  signatureIndices: packed.signatureIndices, signaturesOrAddresses: packed.signaturesOrAddresses, signers,
  attestationsAndSignersDigest: aasDigest.toString(), attestationsAndSignersSignature: sig(aasSig),
  committeeCommitment: keccak256(encodeAbiParameters([{ type: 'address[]' }], [committee.map((a) => a.toString())])),
}, null, 1));
console.log(`proposer index ${proposerIndex}, ${signers.length} signers, digest ${digest}`);
process.exit(0);
