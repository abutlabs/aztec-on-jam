// An epoch's root rollup public inputs and batched blob opening, by Aztec v6's released code
// (@aztec-labs/stdlib, blob-lib 6.0.0, unchanged), for the settlement service's epoch path (#51).
// The batched blob is BatchedBlobAccumulator.batch over the checkpoints' blob fields, and the
// public inputs are RootRollupPublicInputs.toFields(): Aztec's own order of the 113.
//   node epoch.mjs <gen dir> <out prefix> <spec.json>
// spec: {checkpoints: [names, in order], previousArchive, previousInboxRollingHash, chainId,
//        version, vkTreeRoot, protocolContractsHash, proverId}
// writes <out prefix>.pubs.json (113 0x-hex fields), <out prefix>.batched (192 bytes, the L1
// point-evaluation input) and <out prefix>.json (the blob commitments hash, z).
import { RootRollupPublicInputs, EpochConstantData, FeeRecipient } from '@aztec-labs/stdlib/rollup';
import { BatchedBlobAccumulator, getEthBlobEvaluationInputs } from '@aztec-labs/blob-lib';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { EthAddress } from '@aztec-labs/foundation/eth-address';
import { readFileSync, writeFileSync } from 'fs';

const [gen, out, specPath] = process.argv.slice(2);
const spec = JSON.parse(readFileSync(specPath, 'utf8'));
const cps = spec.checkpoints.map(n => JSON.parse(readFileSync(`${gen}/${n}.json`, 'utf8')));
const fields = spec.checkpoints.map(n => JSON.parse(readFileSync(`${gen}/${n}.fields.json`, 'utf8')).map(f => Fr.fromString(f)));
const batched = await BatchedBlobAccumulator.batch(fields, true);
const fr = x => new Fr(BigInt(x));
const pad = (xs, n, empty) => [...xs, ...Array(n - xs.length).fill(0).map(empty)];
const last = cps[cps.length - 1];
const pi = new RootRollupPublicInputs(
  Fr.fromString(spec.previousArchive),
  Fr.fromString(last.archive),
  Fr.fromString(last.outHash),
  Fr.fromString(spec.previousInboxRollingHash),
  Fr.fromString(last.inboxRollingHash),
  pad(cps.map(c => Fr.fromString(c.headerHash)), 32, () => Fr.ZERO),
  pad(cps.map(c => new FeeRecipient(EthAddress.fromString(c.coinbase), Fr.fromString(c.accumulatedFees))), 32, () => new FeeRecipient(EthAddress.ZERO, Fr.ZERO)),
  new EpochConstantData(fr(spec.chainId), fr(spec.version), Fr.fromString(spec.vkTreeRoot), Fr.fromString(spec.protocolContractsHash), fr(spec.proverId)),
  batched.toFinalBlobAccumulator(),
);
const pubs = pi.toFields();
if (pubs.length !== 113) throw new Error(`${pubs.length} public inputs`);
writeFileSync(`${out}.pubs.json`, JSON.stringify(pubs.map(f => f.toString())));
writeFileSync(`${out}.batched`, Buffer.from(getEthBlobEvaluationInputs(batched).slice(2), 'hex'));
writeFileSync(`${out}.json`, JSON.stringify({ blobCommitmentsHash: batched.blobCommitmentsHash.toString(), z: batched.z.toString() }));
console.log(`epoch of ${cps.length} checkpoint(s): 113 public inputs, blob commitments hash ${batched.blobCommitmentsHash}`);
process.exit(0);
