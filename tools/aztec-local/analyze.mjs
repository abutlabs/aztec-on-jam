// #52: what Aztec v6's node asked of its L1, from rpc-proxy.mjs's record. Maps call targets to
// Aztec's contracts (node_getNodeInfo's l1ContractAddresses), selectors to functions and topics
// to events (Aztec's own ABIs, @aztec-foundation/l1-artifacts), and resolves each sent
// transaction on anvil to its target and function.
//   node analyze.mjs <l1.jsonl> <nodeinfo.json> <anvil url> > report.json
import { readFileSync, readdirSync } from 'fs';
import { toFunctionSelector, toEventSelector } from 'viem';

const [logPath, infoPath, anvil] = process.argv.slice(2);
const dir = new URL('./node_modules/@aztec-foundation/l1-artifacts/dest/', import.meta.url).pathname;
const fns = {}, evs = {};
for (const f of readdirSync(dir).filter(f => f.endsWith('Abi.js'))) {
  const mod = await import(dir + f);
  for (const abi of Object.values(mod)) {
    if (!Array.isArray(abi)) continue;
    for (const item of abi) {
      try {
        if (item.type === 'function') fns[toFunctionSelector(item)] ??= item.name;
        if (item.type === 'event') evs[toEventSelector(item)] ??= item.name;
      } catch {}
    }
  }
}
const names = Object.fromEntries(Object.entries(JSON.parse(readFileSync(infoPath, 'utf8')).result.l1ContractAddresses)
  .map(([k, v]) => [v.toLowerCase(), k.replace(/Address$/, '')]));
const who = a => names[a] ?? (a ? `other ${a.slice(0, 10)}` : 'none');
const count = (m, k) => (m[k] = (m[k] ?? 0) + 1);
const methods = {}, calls = {}, logs = {}, storage = {};
const txs = [];
for (const line of readFileSync(logPath, 'utf8').split('\n').filter(Boolean)) {
  const r = JSON.parse(line);
  count(methods, r.m);
  if (r.sel !== undefined) count(calls, `${r.m} ${who(r.to)}.${fns[r.sel] ?? r.sel}`);
  if (r.m === 'eth_getLogs') count(logs, `${r.addr?.map(who).join('+') || 'any'} ${r.topic0?.map(t => evs[t] ?? t.slice(0, 10)).join('|') || 'any event'}`);
  if (r.m === 'eth_getStorageAt') count(storage, who(r.to));
  if (r.m === 'eth_sendRawTransaction' && r.tx) txs.push(r.tx);
}
const sent = {};
for (const h of txs) {
  const res = await fetch(anvil, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_getTransactionByHash', params: [h] }) });
  const t = (await res.json()).result;
  count(sent, t ? `${who(t.to?.toLowerCase())}.${t.to ? fns[t.input.slice(0, 10)] ?? t.input.slice(0, 10) : 'deploy'}${t.blobVersionedHashes?.length ? ` (${t.blobVersionedHashes.length} blobs)` : ''}` : 'unknown');
}
const sort = m => Object.fromEntries(Object.entries(m).sort((a, b) => b[1] - a[1]));
console.log(JSON.stringify({ methods: sort(methods), calls: sort(calls), logs: sort(logs), storage: sort(storage), sent: sort(sent) }, null, 1));
process.exit(0);
