// Checkpoints made by Aztec v6's own released packages (@aztec-labs/stdlib, blob-lib 6.0.0,
// unchanged), for the settlement service (#49, #50). Each checkpoint's blobs are what Aztec's
// publisher sends: getBlobsPerL1Block(checkpoint.toBlobFields()). Its header gets the blobs'
// blobsHash, its slot and the slot's timestamp, the given fee and inbox rolling hash, and its
// hash is Aztec's CheckpointHeader.hash().
//   node gen.mjs <out> <spec.json>
// spec: {genesis (unix), slotSeconds, fee, genesisArchive (0x-hex, or null for a random one),
//        checkpoints: [{name, parent ("genesis" or an earlier name), slot, inbox (0x-hex), txs, sign}],
//        committee (optional): {keys: [0x-hex private keys], seed, epochSlots, chainId, rollup}}
// With a committee, each checkpoint gets <name>.attest, the payload's attestations block, signed
// by Aztec's code (#54): every member signs the payload digest but the one after the proposer;
// sign "noProposer" leaves the proposer unsigned, "badSig" corrupts the proposer's signature.
// For each checkpoint, written to <out>/:
//   <name>.header   348 bytes, ProposedHeaderLib's packed encoding (from header.toViem())
//   <name>.archive  32 bytes, the new archive root
//   <name>.blobs    [n u8][n × (commitment 48 ‖ blob KZG proof 48)]
//   <name>.blob<j>  131,072 bytes, blob j's data
//   <name>.json     Aztec's own values: header hash, blobs hash, slot, number of blobs and txs
//   <name>.fields.json  the checkpoint's blob fields, for an epoch's batched blob (epoch.mjs)
// and genesis.json, the genesis archive root.
import { Checkpoint } from '@aztec-labs/stdlib/checkpoint';
import { CheckpointHeader } from '@aztec-labs/stdlib/rollup';
import { GasFees } from '@aztec-labs/stdlib/gas';
import { CheckpointNumber, SlotNumber } from '@aztec-labs/foundation/branded-types';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { getBlobsPerL1Block, computeBlobsHashFromBlobs, getKzg } from '@aztec-labs/blob-lib';
import { AppendOnlyTreeSnapshot } from '@aztec-labs/stdlib/trees';
import { computeCheckpointPayloadDigest } from '@aztec-labs/stdlib/checkpoint';
import { CommitteeAttestation, CommitteeAttestationsAndSigners } from '@aztec-labs/stdlib/block';
import { getHashedSignaturePayloadTypedData } from '@aztec-labs/stdlib/p2p';
import { Secp256k1Signer } from '@aztec-labs/foundation/crypto/secp256k1-signer';
import { Buffer32 } from '@aztec-labs/foundation/buffer';
import { EthAddress } from '@aztec-labs/foundation/eth-address';
import { encodeAbiParameters, keccak256 } from 'viem';
import { mkdirSync, readFileSync, writeFileSync } from 'fs';

const [out, specPath] = process.argv.slice(2);
const spec = JSON.parse(readFileSync(specPath, 'utf8'));
mkdirSync(out, { recursive: true });
const kzg = getKzg();
const w = (x, n = 32) => Buffer.from(BigInt(x).toString(16).padStart(2 * n, '0'), 'hex');
const hex = b => '0x' + Buffer.from(b).toString('hex');

const genesis = spec.genesisArchive ? new AppendOnlyTreeSnapshot(Fr.fromString(spec.genesisArchive), 1) : AppendOnlyTreeSnapshot.random();
writeFileSync(`${out}/genesis.json`, JSON.stringify({ archive: genesis.root.toString() }));
const archives = { genesis };
// the attestations block (services/aztec-settlement: modifier, indices, signatures or addresses,
// signers, the proposer's signature), signed as Aztec's validators sign
function attest(header, archive, slot, mode) {
  const k = spec.committee, keys = k.keys.map((h) => new Secp256k1Signer(Buffer32.fromString(h)));
  const ctx = { chainId: Number(k.chainId), rollupAddress: EthAddress.fromString(k.rollup) };
  const epoch = slot / BigInt(k.epochSlots);
  const p = Number(BigInt(keccak256(encodeAbiParameters([{ type: 'uint256' }, { type: 'uint256' }, { type: 'uint256' }], [epoch, slot, BigInt(k.seed)]))) % BigInt(keys.length));
  const digest = Buffer32.fromString(computeCheckpointPayloadDigest({ header, archiveRoot: archive, feeAssetPriceModifier: 0n, signatureContext: ctx }).toString());
  const skip = (p + 1) % keys.length;
  const atts = keys.map((key, i) => (i === skip || (mode === 'noProposer' && i === p) ? CommitteeAttestation.fromAddress(key.address) : CommitteeAttestation.fromAddressAndSignature(key.address, key.sign(digest))));
  const aas = new CommitteeAttestationsAndSigners(atts, ctx);
  const packed = aas.getPackedAttestations();
  const s = keys[p].sign(getHashedSignaturePayloadTypedData(aas));
  const sig = Buffer.concat([Buffer.from([s.v]), s.r.toBuffer(), s.s.toBuffer()]);
  if (mode === 'badSig') sig[40] ^= 1;
  const u16 = (n) => Buffer.from([n & 0xff, n >> 8]);
  const idx = Buffer.from(packed.signatureIndices.slice(2), 'hex'), so = Buffer.from(packed.signaturesOrAddresses.slice(2), 'hex');
  const signers = aas.getSigners();
  return Buffer.concat([Buffer.alloc(32), u16(idx.length), idx, u16(so.length), so, u16(signers.length), ...signers.map((a) => a.toBuffer()), sig]);
}
for (const [i, c] of spec.checkpoints.entries()) {
  const slot = BigInt(c.slot);
  const timestamp = BigInt(spec.genesis) + slot * BigInt(spec.slotSeconds);
  const previousArchive = archives[c.parent];
  if (!previousArchive) throw new Error(`${c.name}: unknown parent ${c.parent}`);
  const cp = await Checkpoint.random(CheckpointNumber(i + 1), { numBlocks: 1, txsPerBlock: c.txs ?? 1, previousArchive });
  const blobs = await getBlobsPerL1Block(cp.toBlobFields());
  const header = CheckpointHeader.from({
    ...cp.header,
    blobsHash: computeBlobsHashFromBlobs(blobs),
    inboxRollingHash: Fr.fromString(c.inbox ?? '0x00'),
    slotNumber: SlotNumber(Number(slot)),
    timestamp,
    gasFees: new GasFees(0n, BigInt(spec.fee)),
  });
  const v = header.toViem();
  const packed = Buffer.concat([
    ...[v.lastArchiveRoot, v.blockHeadersHash, v.blobsHash, v.inboxRollingHash, v.outHash].map(x => Buffer.from(x.slice(2), 'hex')),
    w(v.slotNumber), w(v.timestamp, 8), Buffer.from(v.coinbase.slice(2), 'hex'), Buffer.from(v.feeRecipient.slice(2), 'hex'),
    w(v.gasFees.feePerDaGas, 16), w(v.gasFees.feePerL2Gas, 16), w(v.totalManaUsed), w(v.accumulatedFees),
  ]);
  if (packed.length !== 348) throw new Error(`header packs to ${packed.length} bytes`);
  const n = c.name;
  writeFileSync(`${out}/${n}.header`, packed);
  writeFileSync(`${out}/${n}.archive`, cp.archive.root.toBuffer());
  const items = blobs.map(b => Buffer.concat([b.commitment, Buffer.from(kzg.computeBlobKzgProof(b.data, b.commitment))]));
  writeFileSync(`${out}/${n}.blobs`, Buffer.concat([Buffer.from([blobs.length]), ...items]));
  blobs.forEach((b, j) => writeFileSync(`${out}/${n}.blob${j}`, b.data));
  writeFileSync(`${out}/${n}.fields.json`, JSON.stringify(cp.toBlobFields().map(f => f.toString())));
  if (spec.committee) writeFileSync(`${out}/${n}.attest`, attest(header, cp.archive.root, slot, c.sign ?? 'ok'));
  writeFileSync(`${out}/${n}.json`, JSON.stringify({
    headerHash: header.hash().toString(), blobsHash: header.blobsHash.toString(), archive: cp.archive.root.toString(),
    lastArchiveRoot: header.lastArchiveRoot.toString(), slot: Number(slot), timestamp: Number(timestamp),
    blobs: blobs.length, txs: cp.blocks[0].body.txEffects.length, packedHeader: hex(packed),
    outHash: header.epochOutHash.toString(), inboxRollingHash: header.inboxRollingHash.toString(),
    coinbase: header.coinbase.toString(), accumulatedFees: header.accumulatedFees.toString(),
  }, null, 1));
  console.log(`${n}: slot ${slot}, parent ${c.parent}, ${cp.blocks[0].body.txEffects.length} txs, ${blobs.length} blob(s), header hash ${header.hash()}`);
  archives[n] = cp.archive;
}
process.exit(0);
