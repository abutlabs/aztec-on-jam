// Private transactions as Aztec's prover saw them, for spikes/chonk-pvm's tree port (#73): every
// private tx base rollup job in a proof store (on-jam.sh PROOF_STORE=dir) becomes
//   tx-N.ext    the settlement extrinsic for mode 0x83: [u32 len][proof: the kernel's public inputs,
//               then the proof][u32 len][vk][state.cpp's hints]
//   tx-N.state  state_test's input: [the kernel's 1,282 public inputs][hints][the end snapshots
//               Aztec's prover computed, from the job's result]
// in chain order (by the note hash tree's next index), and genesis.bin, the first one's start
// snapshots (mode 0x41). With the node up, each block's last transaction's end snapshots are
// checked against the node's block header: Aztec's own world state, not the prover's arithmetic.
//   node tools/aztec-local/capture-tx.mjs <proof store dir> <out dir> [node url]
import fs from 'node:fs';
import path from 'node:path';
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { PrivateTxBaseRollupPrivateInputs } from '@aztec-labs/stdlib/rollup';
import { ProvingJobResult } from '@aztec-labs/stdlib/interfaces/server';
import { jsonParseWithSchema } from '@aztec-labs/foundation/json-rpc';

const [store, out, nodeUrl = 'http://localhost:8080'] = process.argv.slice(2);
if (!store || !out) { console.log('usage: capture-tx.mjs <proof store dir> <out dir> [node url]'); process.exit(2); }
const KIND = 'PRIVATE_TX_BASE_ROLLUP';
const F = (x) => Buffer.from(BigInt(x.toString()).toString(16).padStart(64, '0'), 'hex');
const u32 = (n) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const snapshots = (s) => [s.noteHashTree, s.nullifierTree, s.publicDataTree].flatMap((t) => [F(t.root), F(t.nextAvailableLeafIndex)]);
const witness = (w) => [F(w.leafIndex), ...w.siblingPath.map(F)];

const dir = path.join(store, 'inputs', KIND);
const ids = fs.existsSync(dir) ? fs.readdirSync(dir) : [];
const txs = ids.map((id) => {
  const inputs = PrivateTxBaseRollupPrivateInputs.fromBuffer(fs.readFileSync(path.join(dir, id)));
  const result = path.join(store, 'outputs', KIND, `${id}.json`);
  const end = fs.existsSync(result) ? jsonParseWithSchema(fs.readFileSync(result, 'utf-8'), ProvingJobResult).result.inputs.endTreeSnapshots : undefined;
  return { id, inputs, end };
}).sort((a, b) => Number(a.inputs.hints.start.noteHashTree.nextAvailableLeafIndex) - Number(b.inputs.hints.start.noteHashTree.nextAvailableLeafIndex));
if (!txs.length) { console.log(`no ${KIND} inputs in ${dir}`); process.exit(1); }
fs.mkdirSync(out, { recursive: true });

let failed = 0;
txs.forEach(({ inputs, end }, n) => {
  const { hidingKernelProofData: data, hints } = inputs;
  const d = hints.treeSnapshotDiffHints;
  const pis = data.publicInputs.toFields();
  const proof = Buffer.concat([...pis, ...data.proof.proof].map(F));
  const vk = data.vkData.vk.keyAsBytes;
  const leaf = hints.feePayerBalanceLeafPreimage;
  const gas = hints.constants.globalVariables.gasFees;
  const entries = d.sortedNullifiers.flatMap((v, i) => v.isZero() ? [] : [Buffer.concat([
    F(v), u32(d.sortedNullifierIndexes[i]),
    F(d.nullifierPredecessorPreimages[i].leaf.nullifier), F(d.nullifierPredecessorPreimages[i].nextKey), F(d.nullifierPredecessorPreimages[i].nextIndex),
    ...witness(d.nullifierPredecessorMembershipWitnesses[i]),
  ])]);
  const hintBytes = Buffer.concat([
    ...snapshots(hints.start), F(gas.feePerDaGas), F(gas.feePerL2Gas),
    F(leaf.leaf.slot), F(leaf.leaf.value), F(leaf.nextKey), F(leaf.nextIndex), ...witness(d.feePayerBalanceMembershipWitness),
    ...d.noteHashSubtreeRootSiblingPath.map(F), ...d.nullifierSubtreeRootSiblingPath.map(F),
    u32(entries.length), ...entries,
  ]);
  fs.writeFileSync(path.join(out, `tx-${n}.ext`), Buffer.concat([u32(proof.length), proof, u32(vk.length), vk, hintBytes]));
  if (end) fs.writeFileSync(path.join(out, `tx-${n}.state`), Buffer.concat([...pis.map(F), hintBytes, ...snapshots(end)]));
  const block = hints.constants.globalVariables.blockNumber;
  console.log(`tx-${n}: block ${block}, ${pis.length} kernel inputs, proof ${proof.length / 32} fields, vk ${vk.length} octets, ` +
    `${entries.length} nullifier(s), note hashes from ${hints.start.noteHashTree.nextAvailableLeafIndex}${end ? '' : ', NOT PROVEN yet (no .state)'}`);
  if (pis.length !== 1282) { console.log('  FAIL: not 1,282 kernel public inputs'); failed++; }
  if (n > 0 && txs[n - 1].end && !txs[n - 1].end.equals(hints.start)) console.log(`  note: start is not tx-${n - 1}'s end (another transaction between them was not captured)`);
});
fs.writeFileSync(path.join(out, 'genesis.bin'), Buffer.concat(snapshots(txs[0].inputs.hints.start)));

// each block's last captured transaction against the node's header for that block
const node = createAztecNodeClient(nodeUrl);
const last = new Map(txs.filter((t) => t.end).map((t) => [Number(t.inputs.hints.constants.globalVariables.blockNumber), t]));
for (const [block, t] of last) {
  let header;
  try { header = await node.getBlockHeader(block); } catch (e) { console.log(`node unreachable (${e.message}); header check skipped`); break; }
  const same = header?.state.partial.equals(t.end);
  console.log(`block ${block}: the prover's end snapshots ${same ? '=' : '!='} the node's header`);
  failed += !same;
}
console.log(failed ? `FAIL ${failed}` : `captured ${txs.length} into ${out}`);
process.exit(failed ? 1 : 0);
