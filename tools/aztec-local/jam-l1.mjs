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
import { AsyncLocalStorage } from 'async_hooks';
import {
  decodeFunctionData, encodeFunctionResult, encodeErrorResult, encodeEventTopics, encodeAbiParameters, keccak256, parseTransaction,
  recoverTransactionAddress, toFunctionSelector, toHex, hexToBytes, bytesToHex, zeroAddress,
} from 'viem';
import * as artifacts from '@aztec-foundation/l1-artifacts';
import { getKzg } from '@aztec-labs/blob-lib';
import { computeAttestationsHash } from '@aztec-labs/ethereum/contracts';
import { computeCheckpointPayloadDigest } from '@aztec-labs/stdlib/checkpoint';
import { CheckpointHeader } from '@aztec-labs/stdlib/rollup';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { EthAddress } from '@aztec-labs/foundation/eth-address';

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
// timestamp is the JAM timeslot as Unix time: cfg's jamEra plus 6 s a slot. The era is JAM's
// common era on a wall-clock chain (PolkaJam's dev chain), and the net's own on one paced from
// slot 0 (nets/jb.sh's) — devnet.sh sets it, and the settlement service's clock uses the same
// field, so the facade and the chain agree to the second.
const slotEpoch = BigInt(C.jamEra ?? 1_735_732_800);
const blocks = [{ number: 0n, hash: '0x' + '00'.repeat(32), slot: 0n, timestamp: BigInt(C.genesisTime), txs: [] }];
let head = blocks[0];
async function poll() {
  try {
    const f = await jip2('finalizedBlock');
    const hash = bytesToHex(Buffer.from(f.header_hash, 'base64'));
    if (hash !== head.hash) {
      const slot = BigInt(f.slot);
      head = { number: BigInt(blocks.length), hash, jamHash: f.header_hash, slot, timestamp: slotEpoch + slot * 6n, txs: [] };
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
// eth_call's state overrides: Aztec's sequencer simulates with the chain tips overridden
// (RollupContract.makeChainTipsOverride writes the STF storage slot, pending << 128 | proven).
const STF_SLOT = keccak256(toHex('aztec.stf.storage'));
function stfOverride(overrides) {
  const o = Object.entries(overrides ?? {}).find(([k]) => k.toLowerCase() === A.rollup)?.[1];
  const d = o?.stateDiff ?? o?.state;
  const v = Array.isArray(d) ? d.find((x) => x.slot?.toLowerCase() === STF_SLOT)?.value : Object.entries(d ?? {}).find(([k]) => k.toLowerCase() === STF_SLOT)?.[1];
  return v ? BigInt(v) : undefined;
}
const callScope = new AsyncLocalStorage();   // {tips, time}: the overrides of the call being answered
const now = () => callScope.getStore()?.time ?? head.timestamp;
async function tips() {
  const tipsOverride = callScope.getStore()?.tips;
  if (tipsOverride) return tipsOverride;
  const p = await read('tip:pending'), q = await read('tip:proven');
  return { pending: p ? u64le(p) : 0n, proven: q ? u64le(q) : 0n };
}
async function checkpointLog(n) {
  if (BigInt(n) === 0n) return { archive: C.genesisArchive, headerHash: word(0), blobCommitmentsHash: word(0), outHash: word(0), slot: 0n, inbox: word(0), consumed: 0n, bucket: 0n };
  const l = await read(keyN('cp:', n));
  if (!l || l.length !== 236) return inFlight(BigInt(n));
  const h = (at) => bytesToHex(l.subarray(at, at + 32));
  return { headerHash: h(0), archive: h(32), slot: u64le(l, 64), outHash: h(72), inbox: h(104), blobCommitmentsHash: h(136), consumed: u64le(l, 168), bucket: u64le(l, 176) };
}
// A checkpoint relayed but not yet accumulated: Aztec's sequencer pipelines, simulating its next
// proposal on top of one still in flight. Walk the chain from the stored tip by parent archive.
async function inFlight(n) {
  const stored = await read('tip:pending');
  let k = stored ? u64le(stored) : 0n;
  if (n <= k) return undefined;
  let tip = await checkpointLog(k);
  while (k < n) {
    const next = [...txs.values()].find((r) => r.propose && r.status === undefined && !r.dropped && r.lastArchive === tip.archive.toLowerCase());
    if (!next) return undefined;
    k++;
    tip = { archive: next.archive, headerHash: next.headerHash, blobCommitmentsHash: word(0), outHash: next.outHash, slot: next.slot, inbox: next.inbox, consumed: 0n, bucket: 0n };
  }
  return tip;
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
    const tx = [...txs.values()].find((x) => x.archive === cp.archive && !x.dropped);
    const topics = encodeEventTopics({ abi: artifacts.RollupAbi, eventName: 'CheckpointProposed', args: { checkpointNumber: n, archive: cp.archive } });
    const ev = artifacts.RollupAbi.find((x) => x.type === 'event' && x.name === 'CheckpointProposed');
    const nonIndexed = ev.inputs.filter((i) => !i.indexed);
    // the hashes the archiver checks the proposal's calldata against (calldata_retriever.js),
    // computed as the Rollup does, by Aztec's own functions
    const byName = { versionedBlobHashes: tx?.blobHashes ?? [], blobHashes: tx?.blobHashes ?? [], payloadDigest: tx?.payloadDigest ?? word(0), attestationsHash: tx?.attestationsHash ?? word(0) };
    const values = nonIndexed.map((i) => byName[i.name] ?? (i.type === 'bytes32' ? word(0) : 0n));
    const data = encodeAbiParameters(nonIndexed, values);
    logs.push({ address: A.rollup, topics, data, blockNumber: b.number, blockHash: b.hash, transactionHash: tx?.hash ?? keccak256(cp.archive), logIndex: logs.length });
    if (tx) { tx.blockNumber = b.number; tx.blockHash = b.hash; tx.status = 1; b.txs.push(tx.hash); }
    log({ checkpoint: Number(n), archive: cp.archive, relayed: !!tx });
  }
  seenCheckpoints = t.pending;
  // relayed transactions the service refused
  for (const tx of txs.values()) {
    if (tx.status === undefined && tx.tag !== undefined && !tx.dropped) {
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
    archive: async () => (await checkpointLog((await tips()).pending))?.archive ?? word(0),
    getCheckpoint: async ([n]) => {
      const cp = await checkpointLog(n);
      if (!cp) return undefined;
      return { archive: cp.archive, headerHash: cp.headerHash, blobCommitmentsHash: cp.blobCommitmentsHash, outHash: cp.outHash, attestationsHash: word(0), payloadDigest: word(0),
        slotNumber: cp.slot, feeHeader: { excessMana: 0n, manaUsed: 0n, ethPerFeeAsset: 10n ** 12n, protocolFee: 0n, proverCost: 0n } };
    },
    status: async ([n]) => {
      const t = await tips();
      const proven = await checkpointLog(t.proven), pend = await checkpointLog(t.pending);
      if (!proven || !pend) revert('Rollup__InvalidArchive', [word(0), word(t.pending)]);
      return [t.proven, proven.archive, t.pending, pend.archive, (await checkpointLog(n))?.archive ?? word(0), proven.slot / BigInt(C.epochSlots)];
    },
    canPruneAtTime: async () => false,
    // ProposeLib's preflight: the slot at _ts and the next checkpoint number, if _archive is the
    // pending tip's and the slot is after its; reverts otherwise
    canProposeAtTime: async ([ts, archive]) => {
      const t = await tips();
      const tip = await checkpointLog(t.pending);
      if (!tip) revert('Rollup__InvalidArchive', [word(0), archive]);
      const slot = slotAt(ts);
      if (archive.toLowerCase() !== tip.archive.toLowerCase()) revert('Rollup__InvalidArchive', [tip.archive, archive]);
      if (slot <= tip.slot) revert('Rollup__SlotAlreadyInChain', [tip.slot, slot]);
      return [slot, t.pending + 1n];
    },
    canPrune: async () => false,
    getCurrentSlot: async () => slotAt(now()),
    getSlotAt: async ([ts]) => slotAt(ts),
    getCurrentEpoch: async () => slotAt(now()) / BigInt(C.epochSlots),
    // ProposeLib.validateHeader, as the service's accumulate applies it (services/aztec-settlement)
    validateHeaderWithAttestations: async ([header]) => { await checkHeader(header); return []; },
    // ProposeLib's header and Inbox checks; returns the bucket to pass as propose's bucketHint
    validateCheckpointHeaderAndInbox: async ([a]) => {
      const t = await tips();
      if (BigInt(a.expectedParentCheckpointNumber) !== t.pending) revert('Rollup__UnexpectedParentCheckpoint', [t.pending, BigInt(a.expectedParentCheckpointNumber)]);
      await checkHeader(a.header, a.flags.ignoreDA ? undefined : a.blobsHash);
      const seq = (await views.inbox.getCurrentBucketSeq());
      for (let s = seq; s >= 0n; s--) {
        const b = await bucketAt(s);
        if (b.totalMsgCount === BigInt(a.expectedTotal)) {
          if (b.rollingHash.toLowerCase() !== a.header.inboxRollingHash.toLowerCase()) revert('Rollup__InvalidInboxRollingHash', [b.rollingHash, a.header.inboxRollingHash]);
          return s;
        }
        if (b.totalMsgCount < BigInt(a.expectedTotal) || s === 0n) break;
      }
      revert('Rollup__InboxTotalNotAtBucketBoundary', [BigInt(a.expectedTotal), (await bucketAt(await views.inbox.getCurrentBucketSeq())).totalMsgCount]);
    },
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
// A revert as a node reports one: JSON-RPC error 3 with Aztec's custom error ABI-encoded, so the
// node's viem decodes it by name (ErrorsAbi).
const revert = (name, args = []) => {
  const data = encodeErrorResult({ abi: artifacts.ErrorsAbi, errorName: name, args });
  throw Object.assign(new Error(`execution reverted: ${name}(${args.join(', ')})`), { revert: true, data });
};
async function checkHeader(h, blobsHash) {
  const t = await tips();
  const tip = await checkpointLog(t.pending);
  if (!tip) revert('Rollup__InvalidArchive', [word(0), h.lastArchiveRoot]);
  const slot = BigInt(h.slotNumber);
  if (h.lastArchiveRoot.toLowerCase() !== tip.archive.toLowerCase()) revert('Rollup__InvalidArchive', [tip.archive, h.lastArchiveRoot]);
  if (slot <= tip.slot) revert('Rollup__SlotAlreadyInChain', [tip.slot, slot]);
  if (BigInt(h.timestamp) !== BigInt(C.genesisTime) + slot * BigInt(C.slotSeconds)) revert('Rollup__InvalidTimestamp', [BigInt(C.genesisTime) + slot * BigInt(C.slotSeconds), BigInt(h.timestamp)]);
  if (h.coinbase === zeroAddress) revert('Rollup__InvalidCoinbase');
  if (BigInt(h.gasFees.feePerDaGas) !== 0n) revert('Rollup__NonZeroDaFee');
  if (BigInt(h.gasFees.feePerL2Gas) !== BigInt(C.minFee)) revert('Rollup__InvalidManaMinFee', [BigInt(C.minFee), BigInt(h.gasFees.feePerL2Gas)]);
  if (blobsHash !== undefined && h.blobsHash.toLowerCase() !== blobsHash.toLowerCase()) revert('Rollup__UnavailableTxs', [h.blobsHash]);
}
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
async function sendRaw(raw) {
  const hash = keccak256(raw);
  const tx = parseTransaction(raw);
  const from = (await recoverTransactionAddress({ serializedTransaction: raw })).toLowerCase();
  const rec = { hash, raw, from, to: tx.to?.toLowerCase(), input: tx.data, nonce: tx.nonce, blobHashes: tx.blobVersionedHashes ?? [], tx };
  // a second transaction for a nonce still pending replaces the first, as in a node's mempool:
  // the first is dropped (never mined); if it was relayed, its relay now belongs to this one
  const prior = [...txs.values()].find((r) => r.from === from && r.nonce === tx.nonce && r.blockNumber === undefined && !r.dropped);
  txs.set(hash, rec);
  if (prior) {
    prior.dropped = true;
    if (prior.propose) {
      Object.assign(rec, { tag: prior.tag, propose: prior.propose, archive: prior.archive, lastArchive: prior.lastArchive, slot: prior.slot, outHash: prior.outHash,
        inbox: prior.inbox, headerHash: prior.headerHash, attestationsHash: prior.attestationsHash, payloadDigest: prior.payloadDigest });
      log({ replaced: prior.hash, by: hash });
      return hash;
    }
  }
  const propose = findPropose(tx);
  if (!propose) { log({ sent: hash, to: nameOf[rec.to] ?? rec.to, note: 'not a proposal: recorded, not relayed' }); rec.status = 1; rec.blockNumber = head.number; rec.blockHash = head.hash; head.txs.push(hash); return hash; }
  rec.tag = tagCounter++ % 256;
  rec.propose = propose;
  const [pa, attestations] = propose.args;
  rec.archive = pa.archive.toLowerCase();
  rec.lastArchive = pa.header.lastArchiveRoot.toLowerCase();
  rec.slot = BigInt(pa.header.slotNumber);
  rec.outHash = pa.header.outHash;
  rec.inbox = pa.header.inboxRollingHash;
  const header = CheckpointHeader.fromViem(pa.header);
  rec.headerHash = header.hash().toString();
  rec.attestationsHash = computeAttestationsHash(attestations);
  rec.payloadDigest = computeCheckpointPayloadDigest({ header, archiveRoot: Fr.fromString(pa.archive), feeAssetPriceModifier: pa.oracleInput.feeAssetPriceModifier,
    signatureContext: { chainId: Number(C.chainId), rollupAddress: EthAddress.fromString(A.rollup) } }).toString();
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
  const [args, attestations, signers, signature, blobsInput] = propose.args;
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
  // the committee's signatures, which refine checks against cfg's committee (#54)
  const u16 = (n) => Buffer.from([n & 0xff, n >> 8]);
  const idx = Buffer.from(hexToBytes(attestations.signatureIndices)), so = Buffer.from(hexToBytes(attestations.signaturesOrAddresses));
  const modifier = Buffer.from(toHex(BigInt.asUintN(256, BigInt(args.oracleInput.feeAssetPriceModifier)), { size: 32 }).slice(2), 'hex');
  const sig = Buffer.concat([Buffer.from([Number(signature.v)]), Buffer.from(hexToBytes(signature.r)), Buffer.from(hexToBytes(signature.s))]);
  const att = Buffer.concat([modifier, u16(idx.length), idx, u16(so.length), so, u16(signers.length), ...signers.map((a) => Buffer.from(hexToBytes(a))), sig]);
  const payload = Buffer.concat([Buffer.from([1, rec.tag]), packed, Buffer.from(hexToBytes(args.archive)), Buffer.from(new BigUint64Array([BigInt(args.bucketHint)]).buffer), Buffer.from([blobs.length]), ...items, att]);
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
  eth_call: async ([c, , overrides]) => {
    const n = stfOverride(overrides);
    return callScope.run({ tips: n === undefined ? undefined : { pending: n >> 128n, proven: n & ((1n << 128n) - 1n) } }, () => ethCall(c.to, c.data ?? c.input));
  },
  eth_estimateGas: () => toHex(10_000_000n),
  // each call through the same handlers as eth_call, with the block's time and state overrides
  eth_simulateV1: async ([req]) => Promise.all((req?.blockStateCalls ?? []).map(async (bsc) => {
    const time = bsc.blockOverrides?.time !== undefined ? BigInt(bsc.blockOverrides.time) : undefined;
    const calls = [];
    for (const c of bsc.calls ?? []) {
      const n = stfOverride(bsc.stateOverrides);
      try {
        const returnData = await callScope.run({ tips: n === undefined ? undefined : { pending: n >> 128n, proven: n & ((1n << 128n) - 1n) }, time }, () => ethCall(c.to, c.data ?? c.input));
        calls.push({ status: '0x1', returnData, gasUsed: toHex(1_000_000n), logs: [] });
      } catch (e) {
        if (!e.revert) log({ error: 'eth_simulateV1', e: String(e).slice(0, 200) });
        calls.push({ status: '0x0', returnData: e.data ?? '0x', gasUsed: toHex(1_000_000n), logs: [], error: { code: 3, message: String(e.message ?? e), data: e.data ?? '0x' } });
      }
    }
    return { ...blockByNumber('latest'), ...(time !== undefined ? { timestamp: toHex(time) } : {}), calls };
  })),
  eth_sendRawTransaction: ([raw]) => sendRaw(raw),
  eth_getTransactionByHash: ([h]) => txObject(txs.get(h)),
  eth_getTransactionReceipt: ([h]) => receipt(txs.get(h)),
  // as a node counts them: "latest" the sender's included transactions, "pending" all it sent
  eth_getTransactionCount: ([a, tag]) => {
    const from = a.toLowerCase();
    const mine = [...txs.values()].filter((r) => r.from === from && !r.dropped);
    const n = tag === 'pending' ? mine.length : mine.filter((r) => r.blockNumber !== undefined).length;
    return toHex(BigInt(n));
  },
  eth_getBalance: () => toHex(10n ** 24n),
  eth_getCode: ([a]) => (Object.values(A).includes(a.toLowerCase()) ? '0x60006000fd' : '0x'),
  eth_getStorageAt: ([a, s]) => {
    if (a.toLowerCase() === A.rollup && s.toLowerCase() === STF_SLOT) {
      return tips().then((t) => toHex((t.pending << 128n) | t.proven, { size: 32 }));
    }
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
    catch (e) {
      if (e.revert) return { jsonrpc: '2.0', id: req.id, error: { code: 3, message: e.message, data: e.data } };
      log({ error: req.method, e: String(e).slice(0, 300) });
      return { jsonrpc: '2.0', id: req.id, error: { code: -32000, message: String(e) } };
    }
  };
  const parsed = JSON.parse(body);
  const out = Array.isArray(parsed) ? await Promise.all(parsed.map(one)) : await one(parsed);
  rs.writeHead(200, { 'content-type': 'application/json' });
  rs.end(JSON.stringify(out, (_, v) => (typeof v === 'bigint' ? toHex(v) : v)));
}).listen(config.port, '127.0.0.1');
await poll();
setInterval(poll, 2000);
console.log(`jam-l1 on :${config.port}, service ${config.service} at ${config.jip2}`);
