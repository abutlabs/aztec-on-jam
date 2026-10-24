// Checkpoints made by Aztec v6's own released packages (@aztec-labs/stdlib, blob-lib 6.0.0,
// unchanged), for the settlement service (#49). Each is chained to the previous one's archive,
// in consecutive Aztec slots, and its blobs are what Aztec's publisher sends:
// getBlobsPerL1Block(checkpoint.toBlobFields()). Its header gets the blobs' blobsHash, the
// slot's timestamp and the given fee and inbox hash, and its hash is Aztec's
// CheckpointHeader.hash(). For each checkpoint i, written to <out>/:
//   cp<i>.header   348 bytes, ProposedHeaderLib's packed encoding (from header.toViem())
//   cp<i>.archive  32 bytes, the new archive root
//   cp<i>.blobs    [n u8][n × (commitment 48 ‖ blob KZG proof 48)]
//   cp<i>.blob<j>  131,072 bytes, blob j's data
//   cp<i>.json     Aztec's own values: header hash, blobs hash, slot, number of blobs and txs
//   node gen.mjs <out> <count> <genesis unix time> <slot seconds> <fee per L2 gas> [txs per block...]
import { Checkpoint } from '@aztec-labs/stdlib/checkpoint';
import { CheckpointHeader } from '@aztec-labs/stdlib/rollup';
import { GasFees } from '@aztec-labs/stdlib/gas';
import { CheckpointNumber, SlotNumber } from '@aztec-labs/foundation/branded-types';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { getBlobsPerL1Block, computeBlobsHashFromBlobs, getKzg } from '@aztec-labs/blob-lib';
import { mkdirSync, writeFileSync } from 'fs';

const [out, count, genesis, slotSecs, fee, ...txs] = process.argv.slice(2);
mkdirSync(out, { recursive: true });
const kzg = getKzg();
const w = (x, n = 32) => Buffer.from(BigInt(x).toString(16).padStart(2 * n, '0'), 'hex');
const hex = b => '0x' + Buffer.from(b).toString('hex');

let previousArchive;
for (let i = 1; i <= Number(count); i++) {
  const slot = BigInt(i);
  const timestamp = BigInt(genesis) + slot * BigInt(slotSecs);
  const txsPerBlock = Number(txs[(i - 1) % Math.max(1, txs.length)] ?? 1);
  const cp = await Checkpoint.random(CheckpointNumber(i), { numBlocks: 1, txsPerBlock, previousArchive });
  const blobs = await getBlobsPerL1Block(cp.toBlobFields());
  const h = cp.header;
  const header = CheckpointHeader.from({
    ...h,
    blobsHash: computeBlobsHashFromBlobs(blobs),
    inboxRollingHash: Fr.ZERO,
    slotNumber: SlotNumber(Number(slot)),
    timestamp,
    gasFees: new GasFees(0n, BigInt(fee)),
  });
  const v = header.toViem();
  const packed = Buffer.concat([
    ...[v.lastArchiveRoot, v.blockHeadersHash, v.blobsHash, v.inboxRollingHash, v.outHash].map(x => Buffer.from(x.slice(2), 'hex')),
    w(v.slotNumber), w(v.timestamp, 8), Buffer.from(v.coinbase.slice(2), 'hex'), Buffer.from(v.feeRecipient.slice(2), 'hex'),
    w(v.gasFees.feePerDaGas, 16), w(v.gasFees.feePerL2Gas, 16), w(v.totalManaUsed), w(v.accumulatedFees),
  ]);
  if (packed.length !== 348) throw new Error(`header packs to ${packed.length} bytes`);
  writeFileSync(`${out}/cp${i}.header`, packed);
  writeFileSync(`${out}/cp${i}.archive`, cp.archive.root.toBuffer());
  const items = blobs.map(b => Buffer.concat([b.commitment, Buffer.from(kzg.computeBlobKzgProof(b.data, b.commitment))]));
  writeFileSync(`${out}/cp${i}.blobs`, Buffer.concat([Buffer.from([blobs.length]), ...items]));
  blobs.forEach((b, j) => writeFileSync(`${out}/cp${i}.blob${j}`, b.data));
  writeFileSync(`${out}/cp${i}.json`, JSON.stringify({
    headerHash: header.hash().toString(), blobsHash: header.blobsHash.toString(), archive: cp.archive.root.toString(),
    lastArchiveRoot: header.lastArchiveRoot.toString(), slot: Number(slot), timestamp: Number(timestamp),
    blobs: blobs.length, txs: cp.blocks[0].body.txEffects.length, packedHeader: hex(packed),
  }, null, 1));
  console.log(`cp${i}: slot ${slot}, ${cp.blocks[0].body.txEffects.length} txs, ${blobs.length} blob(s), header hash ${header.hash()}`);
  previousArchive = cp.archive;
}
process.exit(0);
