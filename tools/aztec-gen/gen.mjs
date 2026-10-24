// Checkpoints made by Aztec v6's own released packages (@aztec-labs/stdlib, blob-lib 6.0.0,
// unchanged), for the settlement service (#49, #50). Each checkpoint's blobs are what Aztec's
// publisher sends: getBlobsPerL1Block(checkpoint.toBlobFields()). Its header gets the blobs'
// blobsHash, its slot and the slot's timestamp, the given fee and inbox rolling hash, and its
// hash is Aztec's CheckpointHeader.hash().
//   node gen.mjs <out> <spec.json>
// spec: {genesis (unix), slotSeconds, fee, genesisArchive (0x-hex, or null for a random one),
//        checkpoints: [{name, parent ("genesis" or an earlier name), slot, inbox (0x-hex), txs}]}
// For each checkpoint, written to <out>/:
//   <name>.header   348 bytes, ProposedHeaderLib's packed encoding (from header.toViem())
//   <name>.archive  32 bytes, the new archive root
//   <name>.blobs    [n u8][n × (commitment 48 ‖ blob KZG proof 48)]
//   <name>.blob<j>  131,072 bytes, blob j's data
//   <name>.json     Aztec's own values: header hash, blobs hash, slot, number of blobs and txs
// and genesis.json, the genesis archive root.
import { Checkpoint } from '@aztec-labs/stdlib/checkpoint';
import { CheckpointHeader } from '@aztec-labs/stdlib/rollup';
import { GasFees } from '@aztec-labs/stdlib/gas';
import { CheckpointNumber, SlotNumber } from '@aztec-labs/foundation/branded-types';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { getBlobsPerL1Block, computeBlobsHashFromBlobs, getKzg } from '@aztec-labs/blob-lib';
import { AppendOnlyTreeSnapshot } from '@aztec-labs/stdlib/trees';
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
  writeFileSync(`${out}/${n}.json`, JSON.stringify({
    headerHash: header.hash().toString(), blobsHash: header.blobsHash.toString(), archive: cp.archive.root.toString(),
    lastArchiveRoot: header.lastArchiveRoot.toString(), slot: Number(slot), timestamp: Number(timestamp),
    blobs: blobs.length, txs: cp.blocks[0].body.txEffects.length, packedHeader: hex(packed),
  }, null, 1));
  console.log(`${n}: slot ${slot}, parent ${c.parent}, ${cp.blocks[0].body.txEffects.length} txs, ${blobs.length} blob(s), header hash ${header.hash()}`);
  archives[n] = cp.archive;
}
process.exit(0);
