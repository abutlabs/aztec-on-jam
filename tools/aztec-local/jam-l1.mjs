// jam-l1: an Ethereum JSON-RPC endpoint that is JAM (#53, docs/aztec/NODE.md). Aztec v6's
// released node uses it as its L1, unchanged. Reads come from the settlement service's storage
// over JIP-2. A proposal (a blob transaction to Multicall3 wrapping Rollup.propose) becomes a
// work package, submitted with `jamboree rpc submit`, its blobs as extrinsics. "L1 blocks" are
// JAM's finalized blocks; logs, receipts and traces are synthesized from what the service
// accepted.
//   node jam-l1.mjs <config.json>
// config: {port, jip2 (ws url), chainSpec, jamboree, service, cfg: {genesisTime, slotSeconds,
//   epochSlots, minFee, manaLimit, chainId, version, vkTreeRoot, protocolContractsHash,
//   genesisArchive, proofSubmissionEpochs}, addresses: {registry, rollup, inbox, outbox, ...},
//   work (a directory for packages), log}
// Every request is logged to config.log; a read it has no handler for answers zero values of the
// right ABI shape and is logged as "unhandled", so a run lists what is missing.
import { createServer } from 'http';
import { appendFileSync, mkdirSync, writeFileSync, readFileSync } from 'fs';
import { spawn } from 'child_process';
import {
  decodeFunctionData, encodeFunctionResult, encodeEventTopics, encodeAbiParameters, keccak256, parseTransaction,
  recoverTransactionAddress, toFunctionSelector, toHex, hexToBytes, bytesToHex, zeroAddress,
} from 'viem';
import * as artifacts from '@aztec-foundation/l1-artifacts';
import { getKzg } from '@aztec-labs/blob-lib';

const config = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const C = config.cfg, A = Object.fromEntries(Object.entries(config.addresses).map(([k, v]) => [k, v.toLowerCase()]));
mkdirSync(config.work, { recursive: true });
const log = (o) => appendFileSync(config.log, JSON.stringify({ t: Date.now(), ...o }) + '\n');
const b64 = (b) => Buffer.from(b).toString('base64');
const u64le = (b, at = 0) => Buffer.from(b).readBigUInt64LE(at);
const word = (n) => toHex(BigInt(n), { size: 32 });

// --- JIP-2 ---------------------------------------------------------------------------------
let ws, nextId = 0;
const pending = new Map();
function connect() {
  return new Promise((resolve, reject) => {
    ws = new WebSocket(config.jip2);
    ws.onopen = () => resolve();
    ws.onerror = (e) => reject(e);
    ws.onmessage = (m) => {
      const msg = JSON.parse(m.data);
      const p = pending.get(msg.id);
      if (p) { pending.delete(msg.id); msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result); }
    };
    ws.onclose = () => { ws = undefined; };
  });
}
async function jip2(method, ...params) {
  if (!ws) await connect();
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
  });
}

// --- JAM's head as "L1 blocks" ------------------------------------------------------------
// L1 block n is the n-th finalized JAM block this facade saw, starting from 1 at its start. Its
// timestamp is the JAM timeslot as Unix time (GP 0.8.0 overview.tex:91).
const JAM_COMMON_ERA = 1_735_732_800n;
const blocks = [{ number: 0n, hash: '0x' + '00'.repeat(32), slot: 0n, timestamp: BigInt(C.genesisTime), txs: [] }];
let head = blocks[0];
async function poll() {
  try {
    const f = await jip2('finalizedBlock');
    const hash = bytesToHex(Buffer.from(f.header_hash, 'base64'));
    if (hash !== head.hash) {
      const slot = BigInt(f.slot);
      head = { number: BigInt(blocks.length), hash, jamHash: f.header_hash, slot, timestamp: JAM_COMMON_ERA + slot * 6n, txs: [] };
      blocks.push(head);
      await onNewBlock(head);
    }
  } catch (e) { log({ poll: String(e) }); }
}
async function read(key) {
  const v = await jip2('serviceValue', head.jamHash ?? b64(Buffer.alloc(32)), config.service, b64(key));
  return v ? Buffer.from(v, 'base64') : undefined;
}
const keyN = (prefix, n) => Buffer.concat([Buffer.from(prefix), Buffer.from(new BigUint64Array([BigInt(n)]).buffer)]);

// --- The service's state, as Aztec's contracts would report it -------------------------------
async function tips() {
  const p = await read('tip:pending'), q = await read('tip:proven');
  return { pending: p ? u64le(p) : 0n, proven: q ? u64le(q) : 0n };
}
async function checkpointLog(n) {
  if (BigInt(n) === 0n) return { archive: C.genesisArchive, headerHash: word(0), blobCommitmentsHash: word(0), outHash: word(0), slot: 0n, inbox: word(0), consumed: 0n, bucket: 0n };
  const l = await read(keyN('cp:', n));
  if (!l || l.length !== 236) return undefined;
  const h = (at) => bytesToHex(l.subarray(at, at + 32));
  return { headerHash: h(0), archive: h(32), slot: u64le(l, 64), outHash: h(72), inbox: h(104), blobCommitmentsHash: h(136), consumed: u64le(l, 168), bucket: u64le(l, 176) };
}
const slotAt = (ts) => (BigInt(ts) - BigInt(C.genesisTime)) / BigInt(C.slotSeconds);

// --- Logs and transactions it synthesizes ------------------------------------------------
const logs = [];          // {address, topics, data, blockNumber, blockHash, transactionHash, logIndex}
const txs = new Map();    // hash -> relayed transaction
let seenCheckpoints = 0n;
// What deployment would have left in block 0: the Rollup's Ownable constructor event, which the
// archiver probes to check the endpoint keeps log history (validate_historical_logs.js).
logs.push({ address: A.rollup, topics: encodeEventTopics({ abi: artifacts.RollupAbi, eventName: 'OwnershipTransferred', args: { previousOwner: zeroAddress, newOwner: A.governance } }),
  data: '0x', blockNumber: 0n, blockHash: blocks[0].hash, transactionHash: keccak256(A.rollup), logIndex: 0 });
async function onNewBlock(b) {
  const t = await tips();
  for (let n = seenCheckpoints + 1n; n <= t.pending; n++) {
    const cp = await checkpointLog(n);
    const tx = [...txs.values()].find((x) => x.archive === cp.archive);
    const topics = encodeEventTopics({ abi: artifacts.RollupAbi, eventName: 'CheckpointProposed', args: { checkpointNumber: n, archive: cp.archive } });
    const ev = artifacts.RollupAbi.find((x) => x.type === 'event' && x.name === 'CheckpointProposed');
    const nonIndexed = ev.inputs.filter((i) => !i.indexed);
    const values = nonIndexed.map((i) => (i.name === 'versionedBlobHashes' || i.name === 'blobHashes' ? tx?.blobHashes ?? [] : i.type === 'bytes32' ? word(0) : 0n));
    const data = encodeAbiParameters(nonIndexed, values);
    logs.push({ address: A.rollup, topics, data, blockNumber: b.number, blockHash: b.hash, transactionHash: tx?.hash ?? keccak256(cp.archive), logIndex: logs.length });
    if (tx) { tx.blockNumber = b.number; tx.blockHash = b.hash; tx.status = 1; b.txs.push(tx.hash); }
    log({ checkpoint: Number(n), archive: cp.archive, relayed: !!tx });
  }
  seenCheckpoints = t.pending;
  // relayed transactions the service refused
  for (const tx of txs.values()) {
    if (tx.status === undefined && tx.tag !== undefined) {
      const v = await read(Buffer.from([0x76, 0x3a, tx.tag]));
      if (v && v[0] !== 1) { tx.status = 0; tx.blockNumber = b.number; tx.blockHash = b.hash; b.txs.push(tx.hash); log({ refused: tx.hash, reason: v[0] }); }
    }
  }
}

// --- Contract reads ------------------------------------------------------------------------
const abiOf = { [A.rollup]: artifacts.RollupAbi, [A.inbox]: artifacts.InboxAbi, [A.outbox]: artifacts.OutboxAbi, [A.registry]: artifacts.RegistryAbi,
  [A.governance]: artifacts.GovernanceAbi, [A.governanceProposer]: artifacts.GovernanceProposerAbi, [A.feeJuice]: artifacts.TestERC20Abi,
  [A.stakingAsset]: artifacts.TestERC20Abi, [A.feeJuicePortal]: artifacts.FeeJuicePortalAbi, [A.gse]: artifacts.GSEAbi,
  [A.rewardDistributor]: artifacts.RewardDistributorAbi, [A.coinIssuer]: artifacts.CoinIssuerAbi };
const views = {
  rollup: {
    getTips: async () => ({ pending: (await tips()).pending, proven: (await tips()).proven }),
    getPendingCheckpointNumber: async () => (await tips()).pending,
    getProvenCheckpointNumber: async () => (await tips()).proven,
    archiveAt: async ([n]) => (await checkpointLog(n))?.archive ?? word(0),
    archive: async () => (await checkpointLog((await tips()).pending)).archive,
    getCheckpoint: async ([n]) => {
      const cp = await checkpointLog(n);
      if (!cp) return undefined;
      return { archive: cp.archive, headerHash: cp.headerHash, blobCommitmentsHash: cp.blobCommitmentsHash, outHash: cp.outHash, attestationsHash: word(0), payloadDigest: word(0),
        slotNumber: cp.slot, feeHeader: { excessMana: 0n, manaUsed: 0n, ethPerFeeAsset: 10n ** 12n, protocolFee: 0n, proverCost: 0n } };
    },
    status: async ([n]) => {
      const t = await tips();
      return [t.proven, (await checkpointLog(t.proven)).archive, t.pending, (await checkpointLog(t.pending)).archive, (await checkpointLog(n))?.archive ?? word(0), (await checkpointLog(t.proven)).slot / BigInt(C.epochSlots)];
    },
    canPruneAtTime: async () => false,
    canPrune: async () => false,
    getCurrentSlot: async () => slotAt(head.timestamp),
    getSlotAt: async ([ts]) => slotAt(ts),
    getCurrentEpoch: async () => slotAt(head.timestamp) / BigInt(C.epochSlots),
    getEpochAt: async ([ts]) => slotAt(ts) / BigInt(C.epochSlots),
    getEpochForCheckpoint: async ([n]) => ((await checkpointLog(n))?.slot ?? 0n) / BigInt(C.epochSlots),
    getTimestampForSlot: async ([s]) => BigInt(C.genesisTime) + BigInt(s) * BigInt(C.slotSeconds),
    getGenesisTime: async () => BigInt(C.genesisTime),
    getSlotDuration: async () => BigInt(C.slotSeconds),
    getEpochDuration: async () => BigInt(C.epochSlots),
    getProofSubmissionEpochs: async () => BigInt(C.proofSubmissionEpochs),
    L1_BLOCK_AT_GENESIS: async () => 0n,
    getManaMinFeeAt: async () => BigInt(C.minFee),
    getManaLimit: async () => BigInt(C.manaLimit),
    getManaTarget: async () => BigInt(C.manaLimit) / 2n,
    getL1FeesAt: async () => ({ baseFee: 0n, blobFee: 0n }),
    getProtocolFeeMargin: async () => 0n,
    getProvingCostPerManaInEth: async () => 0n,
    getVersion: async () => BigInt(C.version),
    getInbox: async () => A.inbox,
    getOutbox: async () => A.outbox,
    getFeeAsset: async () => A.feeJuice,
    getFeeAssetPortal: async () => A.feeJuicePortal,
    getRewardDistributor: async () => A.rewardDistributor,
    getStakingAsset: async () => A.stakingAsset,
    getGSE: async () => A.gse,
    getCommitteeAt: async () => [],
    getEpochCommittee: async () => [],
    getCurrentEpochCommittee: async () => [],
    getTargetCommitteeSize: async () => 0n,
    getSampleSeedAt: async () => 0n,
    getCurrentSampleSeed: async () => 0n,
    getCurrentProposer: async () => zeroAddress,
    getProposerAt: async () => zeroAddress,
    getEscapeHatch: async () => zeroAddress,
    getSlasher: async () => zeroAddress,
    getLagInEpochsForValidatorSet: async () => 2n,
    getLagInEpochsForRandao: async () => 1n,
    getVkTreeRoot: async () => C.vkTreeRoot,
    getProtocolContractsHash: async () => C.protocolContractsHash,
    getGenesisArchiveTreeRoot: async () => C.genesisArchive,
    getAttesterCountAtTime: async () => 0n,
    getAttesters: async () => [],
    isEscapeHatchOpen: async () => false,
  },
  inbox: {
    getState: async () => {
      const seq = await read('inbox:seq');
      const s = seq ? u64le(seq) : 0n;
      const b = s ? await read(keyN('inbox:b:', s)) : undefined;
      return { rollingHash: b ? bytesToHex(b.subarray(0, 32)) : word(0), totalMessagesInserted: b ? u64le(b, 32) : 0n, currentBucketSeq: s };
    },
    getCurrentBucketSeq: async () => { const seq = await read('inbox:seq'); return seq ? u64le(seq) : 0n; },
    getBucket: async ([s]) => bucketAt(s),
    getBucketAtOrBeforeTotal: async ([upper]) => {
      const seq = await read('inbox:seq');
      for (let s = seq ? u64le(seq) : 0n; s >= 0n; s--) {
        const b = await bucketAt(s);
        if (b.totalMsgCount <= BigInt(upper)) return [s, b];
        if (s === 0n) break;
      }
      return [0n, await bucketAt(0n)];
    },
    getTotalMessagesInserted: async () => (await views.inbox.getState()).totalMessagesInserted,
    getFeeAssetPortal: async () => A.feeJuicePortal,
  },
  registry: { getRollup: async () => A.rollup, getCanonicalRollup: async () => A.rollup, getGovernance: async () => A.governance, numberOfVersions: async () => 1n },
  governance: { governanceProposer: async () => A.governanceProposer },
  governanceProposer: { ROUND_SIZE: async () => 10n, QUORUM_SIZE: async () => 6n, getInstance: async () => A.rollup },
  feeJuice: { owner: async () => A.governance, balanceOf: async () => 0n },
};
async function bucketAt(s) {
  const b = BigInt(s) ? await read(keyN('inbox:b:', s)) : undefined;
  return b ? { rollingHash: bytesToHex(b.subarray(0, 32)), totalMsgCount: u64le(b, 32), timestamp: u64le(b, 40), msgCount: BigInt(b.readUInt32LE(48)) }
    : { rollingHash: word(0), totalMsgCount: 0n, timestamp: 0n, msgCount: 0n };
}
const nameOf = Object.fromEntries(Object.entries(A).map(([k, v]) => [v, k]));
async function ethCall(to, data) {
  to = to?.toLowerCase();
  const abi = abiOf[to];
  if (!abi) { log({ unhandled: 'eth_call', to, sel: data?.slice(0, 10) }); return '0x'; }
  let fn;
  try { fn = decodeFunctionData({ abi, data }); } catch { log({ unhandled: 'selector', to: nameOf[to], sel: data.slice(0, 10) }); return '0x'; }
  const h = views[nameOf[to]]?.[fn.functionName];
  const item = abi.find((x) => x.type === 'function' && x.name === fn.functionName && toFunctionSelector(x) === data.slice(0, 10));
  let result = h ? await h(fn.args ?? []) : undefined;
  if (!h) log({ unhandled: 'view', contract: nameOf[to], fn: fn.functionName });
  if (result === undefined) result = zeroOf(item.outputs);
  return encodeFunctionResult({ abi: [item], functionName: fn.functionName, result });
}
function zeroOf(outputs) {
  const z = (p) => p.type === 'tuple' ? Object.fromEntries(p.components.map((c) => [c.name, z(c)])) : p.type.endsWith(']') ? [] :
    p.type === 'address' ? zeroAddress : p.type === 'bool' ? false : p.type.startsWith('bytes') ? (p.type === 'bytes' ? '0x' : toHex(0n, { size: Number(p.type.slice(5)) })) : p.type === 'string' ? '' : 0n;
  return outputs.length === 1 ? z(outputs[0]) : outputs.map(z);
}

// --- Writes: a proposal becomes a work package ---------------------------------------------
let tagCounter = 0;
const nonces = new Map();
async function sendRaw(raw) {
  const hash = keccak256(raw);
  const tx = parseTransaction(raw);
  const from = (await recoverTransactionAddress({ serializedTransaction: raw })).toLowerCase();
  nonces.set(from, (nonces.get(from) ?? 0n) + 1n);
  const rec = { hash, raw, from, to: tx.to?.toLowerCase(), input: tx.data, nonce: tx.nonce, blobHashes: tx.blobVersionedHashes ?? [], tx };
  txs.set(hash, rec);
  const propose = findPropose(tx);
  if (!propose) { log({ sent: hash, to: nameOf[rec.to] ?? rec.to, note: 'not a proposal: recorded, not relayed' }); rec.status = 1; rec.blockNumber = head.number; rec.blockHash = head.hash; head.txs.push(hash); return hash; }
  rec.tag = tagCounter++ % 256;
  rec.archive = propose.args[0].archive.toLowerCase();
  await relayPropose(rec, propose, tx);
  return hash;
}
function findPropose(tx) {
  const tryRollup = (data) => { try { const d = decodeFunctionData({ abi: artifacts.RollupAbi, data }); return d.functionName === 'propose' ? d : undefined; } catch { return undefined; } };
  if (tx.to?.toLowerCase() === A.rollup) return tryRollup(tx.data);
  if (tx.to?.toLowerCase() === A.multicall3) {
    const mc = decodeFunctionData({ abi: MULTICALL3, data: tx.data });
    for (const c of mc.args[0]) if (c.target.toLowerCase() === A.rollup) { const d = tryRollup(c.callData); if (d) return d; }
  }
  return undefined;
}
async function relayPropose(rec, propose, tx) {
  const [args, , , , blobsInput] = propose.args;
  const h = args.header;
  const be = (v, n) => Buffer.from(BigInt(v).toString(16).padStart(2 * n, '0'), 'hex');
  const packed = Buffer.concat([
    ...[h.lastArchiveRoot, h.blockHeadersHash, h.blobsHash, h.inboxRollingHash, h.outHash].map((x) => Buffer.from(hexToBytes(x))),
    be(h.slotNumber, 32), be(h.timestamp, 8), Buffer.from(hexToBytes(h.coinbase)), Buffer.from(hexToBytes(h.feeRecipient)),
    be(h.gasFees.feePerDaGas, 16), be(h.gasFees.feePerL2Gas, 16), be(h.totalManaUsed, 32), be(h.accumulatedFees, 32),
  ]);
  const blobs = (tx.sidecars ?? []).map((s) => Buffer.from(hexToBytes(s.blob)));
  const commitments = Buffer.from(hexToBytes(blobsInput)).subarray(1);
  const kzg = getKzg();
  const items = blobs.map((b, i) => { const c = commitments.subarray(48 * i, 48 * i + 48); return Buffer.concat([c, Buffer.from(kzg.computeBlobKzgProof(b, c))]); });
  const payload = Buffer.concat([Buffer.from([1, rec.tag]), packed, Buffer.from(hexToBytes(args.archive)), Buffer.from(new BigUint64Array([BigInt(args.bucketHint)]).buffer), Buffer.from([blobs.length]), ...items]);
  const dir = `${config.work}/${rec.hash}`;
  mkdirSync(dir, { recursive: true });
  writeFileSync(`${dir}/payload.bin`, payload);
  blobs.forEach((b, i) => writeFileSync(`${dir}/blob${i}`, b));
  const argv = ['rpc', '--url', config.jip2, '--chain-spec', config.chainSpec, 'submit', String(config.service), `@${dir}/payload.bin`,
    ...blobs.flatMap((_, i) => ['--extrinsic', `@${dir}/blob${i}`]), '--exports', String(32 * blobs.length)];
  const p = spawn(config.jamboree, argv, { stdio: ['ignore', 'pipe', 'pipe'] });
  let out = '';
  p.stdout.on('data', (d) => (out += d)); p.stderr.on('data', (d) => (out += d));
  p.on('close', (code) => log({ submitted: rec.hash, tag: rec.tag, blobs: blobs.length, code, out: out.slice(-300) }));
  log({ relaying: rec.hash, tag: rec.tag, slot: String(h.slotNumber), blobs: blobs.length });
}
const MULTICALL3 = [{ type: 'function', name: 'aggregate3', stateMutability: 'payable', inputs: [{ name: 'calls', type: 'tuple[]', components: [{ name: 'target', type: 'address' }, { name: 'allowFailure', type: 'bool' }, { name: 'callData', type: 'bytes' }] }],
  outputs: [{ name: 'returnData', type: 'tuple[]', components: [{ name: 'success', type: 'bool' }, { name: 'returnData', type: 'bytes' }] }] }];

// --- Blocks, receipts, traces, fees ----------------------------------------------------------
function blockByNumber(tag, full) {
  const b = tag === 'latest' || tag === 'finalized' || tag === 'safe' || tag === 'pending' ? head : blocks[Number(BigInt(tag))];
  if (!b) return null;
  const n = Number(b.number);
  return {
    number: toHex(b.number), hash: b.hash, parentHash: n ? blocks[n - 1].hash : '0x' + '00'.repeat(32), timestamp: toHex(b.timestamp),
    nonce: '0x0000000000000000', sha3Uncles: '0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347', logsBloom: '0x' + '00'.repeat(256),
    transactionsRoot: b.hash, stateRoot: b.hash, receiptsRoot: b.hash, miner: zeroAddress, difficulty: '0x0', totalDifficulty: '0x0', extraData: '0x',
    size: '0x1', gasLimit: toHex(60_000_000n), gasUsed: '0x0', baseFeePerGas: '0x1', blobGasUsed: '0x0', excessBlobGas: '0x0',
    parentBeaconBlockRoot: '0x' + '00'.repeat(32), withdrawalsRoot: '0x' + '00'.repeat(32), withdrawals: [], uncles: [], mixHash: b.hash,
    transactions: full ? b.txs.map((h) => txObject(txs.get(h))) : b.txs,
  };
}
function txObject(r) {
  if (!r) return null;
  const t = r.tx;
  return { hash: r.hash, from: r.from, to: r.to, input: r.input ?? '0x', nonce: toHex(t.nonce ?? 0), gas: toHex(t.gas ?? 0n), value: toHex(t.value ?? 0n),
    type: toHex(t.type === 'eip4844' ? 3 : t.type === 'eip1559' ? 2 : 0), chainId: toHex(BigInt(C.chainId)), maxFeePerGas: toHex(t.maxFeePerGas ?? 1n),
    maxPriorityFeePerGas: toHex(t.maxPriorityFeePerGas ?? 1n), maxFeePerBlobGas: toHex(t.maxFeePerBlobGas ?? 1n), gasPrice: toHex(t.maxFeePerGas ?? 1n),
    blobVersionedHashes: r.blobHashes, accessList: [], v: '0x0', r: '0x0', s: '0x0', yParity: '0x0',
    blockNumber: r.blockNumber !== undefined ? toHex(r.blockNumber) : null, blockHash: r.blockHash ?? null, transactionIndex: r.blockNumber !== undefined ? '0x0' : null };
}
function receipt(r) {
  if (!r || r.blockNumber === undefined) return null;
  const ls = logs.filter((l) => l.transactionHash === r.hash).map(logObject);
  return { transactionHash: r.hash, transactionIndex: '0x0', blockHash: r.blockHash, blockNumber: toHex(r.blockNumber), from: r.from, to: r.to,
    cumulativeGasUsed: toHex(1_000_000n), gasUsed: toHex(1_000_000n), effectiveGasPrice: '0x1', blobGasUsed: toHex(131072n * BigInt(r.blobHashes.length)),
    blobGasPrice: '0x1', contractAddress: null, logs: ls, logsBloom: '0x' + '00'.repeat(256), status: toHex(r.status ?? 0), type: '0x3' };
}
const logObject = (l) => ({ address: l.address, topics: l.topics, data: l.data, blockNumber: toHex(l.blockNumber), blockHash: l.blockHash,
  transactionHash: l.transactionHash, transactionIndex: '0x0', logIndex: toHex(l.logIndex), removed: false });
function trace(r) {
  if (!r) return null;
  const inner = r.to === A.multicall3 ? decodeFunctionData({ abi: MULTICALL3, data: r.input }).args[0].map((c) => ({ type: 'CALL', from: A.multicall3, to: c.target.toLowerCase(), input: c.callData, output: '0x', gas: '0x0', gasUsed: '0x0', value: '0x0' })) : [];
  return { type: 'CALL', from: r.from, to: r.to, input: r.input, output: '0x', gas: '0x0', gasUsed: '0x0', value: '0x0', calls: inner };
}
function parityTrace(r) {
  const t = trace(r);
  if (!t) return null;
  const one = (c, addr) => ({ action: { callType: 'call', from: c.from, to: c.to, input: c.input, gas: '0x0', value: '0x0' }, result: { gasUsed: '0x0', output: '0x' }, subtraces: c.calls?.length ?? 0, traceAddress: addr, type: 'call',
    transactionHash: r.hash, blockNumber: Number(r.blockNumber ?? 0), blockHash: r.blockHash });
  return [one(t, []), ...(t.calls ?? []).map((c, i) => one(c, [i]))];
}

// --- JSON-RPC --------------------------------------------------------------------------------
const methods = {
  eth_chainId: () => toHex(BigInt(C.chainId)),
  net_version: () => String(C.chainId),
  web3_clientVersion: () => 'jam-l1/0.0.1',
  eth_blockNumber: () => toHex(head.number),
  eth_getBlockByNumber: ([tag, full]) => blockByNumber(tag, full),
  eth_getBlockByHash: ([h, full]) => { const b = blocks.find((x) => x.hash === h); return b ? blockByNumber(toHex(b.number), full) : null; },
  eth_call: ([c]) => ethCall(c.to, c.data ?? c.input),
  eth_estimateGas: () => toHex(10_000_000n),
  eth_simulateV1: ([req]) => (req?.blockStateCalls ?? []).map(() => ({ ...blockByNumber('latest'), calls: [{ status: '0x1', returnData: '0x', gasUsed: toHex(1_000_000n), logs: [] }] })),
  eth_sendRawTransaction: ([raw]) => sendRaw(raw),
  eth_getTransactionByHash: ([h]) => txObject(txs.get(h)),
  eth_getTransactionReceipt: ([h]) => receipt(txs.get(h)),
  eth_getTransactionCount: ([a]) => toHex(nonces.get(a.toLowerCase()) ?? 0n),
  eth_getBalance: () => toHex(10n ** 24n),
  eth_getCode: ([a]) => (Object.values(A).includes(a.toLowerCase()) ? '0x60006000fd' : '0x'),
  eth_getStorageAt: ([a, s]) => {
    // RollupContract.isEpochSetUp reads validator selection storage, epoch => committee commitment,
    // straight from storage. With no committee, every epoch is set up.
    if (a.toLowerCase() === A.rollup) {
      const base = BigInt(keccak256(toHex('aztec.validator_selection.storage')));
      const current = slotAt(head.timestamp) / BigInt(C.epochSlots);
      for (let e = current + 4n; e >= 0n; e--) {
        if (keccak256(encodeAbiParameters([{ type: 'uint256' }, { type: 'uint256' }], [e, base])) === s.toLowerCase()) return word(1);
        if (e === 0n) break;
      }
    }
    log({ unhandled: 'eth_getStorageAt', to: nameOf[a.toLowerCase()] ?? a, slot: s });
    return word(0);
  },
  eth_getLogs: ([f]) => {
    const from = f.fromBlock && !['latest', 'earliest', 'pending', 'finalized', 'safe'].includes(f.fromBlock) ? BigInt(f.fromBlock) : 0n;
    const to = f.toBlock && !['latest', 'pending', 'finalized', 'safe'].includes(f.toBlock) ? BigInt(f.toBlock) : head.number;
    const addrs = [f.address].flat().filter(Boolean).map((a) => a.toLowerCase());
    const t0 = [f.topics?.[0]].flat().filter(Boolean);
    return logs.filter((l) => l.blockNumber >= from && l.blockNumber <= to && (!addrs.length || addrs.includes(l.address)) && (!t0.length || t0.includes(l.topics[0]))).map(logObject);
  },
  eth_feeHistory: ([n]) => ({ oldestBlock: toHex(head.number), baseFeePerGas: Array(Number(BigInt(n)) + 1).fill('0x1'), gasUsedRatio: Array(Number(BigInt(n))).fill(0),
    baseFeePerBlobGas: Array(Number(BigInt(n)) + 1).fill('0x1'), blobGasUsedRatio: Array(Number(BigInt(n))).fill(0), reward: Array(Number(BigInt(n))).fill(['0x1']) }),
  eth_blobBaseFee: () => '0x1',
  eth_maxPriorityFeePerGas: () => '0x1',
  eth_gasPrice: () => '0x1',
  eth_syncing: () => false,
  debug_traceTransaction: ([h]) => trace(txs.get(h)),
  trace_transaction: ([h]) => parityTrace(txs.get(h)),
};
createServer(async (rq, rs) => {
  let body = '';
  for await (const c of rq) body += c;
  const one = async (req) => {
    const m = methods[req.method];
    if (!m) { log({ unhandled: req.method, params: JSON.stringify(req.params).slice(0, 200) }); return { jsonrpc: '2.0', id: req.id, error: { code: -32601, message: `jam-l1: ${req.method} is not supported` } }; }
    try { return { jsonrpc: '2.0', id: req.id, result: await m(req.params ?? []) }; }
    catch (e) { log({ error: req.method, e: String(e).slice(0, 300) }); return { jsonrpc: '2.0', id: req.id, error: { code: -32000, message: String(e) } }; }
  };
  const parsed = JSON.parse(body);
  const out = Array.isArray(parsed) ? await Promise.all(parsed.map(one)) : await one(parsed);
  rs.writeHead(200, { 'content-type': 'application/json' });
  rs.end(JSON.stringify(out, (_, v) => (typeof v === 'bigint' ? toHex(v) : v)));
}).listen(config.port, '127.0.0.1');
await poll();
setInterval(poll, 2000);
console.log(`jam-l1 on :${config.port}, service ${config.service} at ${config.jip2}`);
