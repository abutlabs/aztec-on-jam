// A JSON-RPC proxy that records what Aztec's node asks of its L1 (#52): every method, and for
// eth_call / eth_estimateGas the target and 4-byte selector, for eth_getLogs the address and
// topic0, for eth_sendRawTransaction the transaction hash (resolved to its target and
// selector afterwards). One JSON line per request to <log>; responses pass through unchanged.
//   node rpc-proxy.mjs <listen port> <upstream url> <log>
import { createServer } from 'http';
import { appendFileSync } from 'fs';

const [port, upstream, log] = process.argv.slice(2);
const note = (req, res) => {
  const p = req.params ?? [];
  const r = { t: Date.now(), m: req.method };
  if ((req.method === 'eth_call' || req.method === 'eth_estimateGas' || req.method === 'eth_createAccessList') && p[0]) {
    r.to = p[0].to?.toLowerCase(); r.sel = (p[0].data ?? p[0].input ?? '').slice(0, 10);
  } else if (req.method === 'eth_getLogs' && p[0]) {
    r.addr = [p[0].address].flat().filter(Boolean).map(a => a.toLowerCase()); r.topic0 = [p[0].topics?.[0]].flat().filter(Boolean);
  } else if (req.method === 'eth_sendRawTransaction' && res?.result) {
    r.tx = res.result;
  } else if (req.method === 'eth_getStorageAt' && p[0]) {
    r.to = p[0].toLowerCase();
  } else if ((req.method === 'debug_traceTransaction' || req.method === 'trace_transaction') && p[0]) {
    r.tx = p[0];
  }
  appendFileSync(log, JSON.stringify(r) + '\n');
};
createServer(async (rq, rs) => {
  let body = '';
  for await (const c of rq) body += c;
  const up = await fetch(upstream, { method: 'POST', headers: { 'content-type': 'application/json' }, body });
  const text = await up.text();
  try {
    const reqs = [JSON.parse(body)].flat(), resps = [JSON.parse(text)].flat();
    reqs.forEach(r => note(r, resps.find(x => x.id === r.id)));
  } catch {}
  rs.writeHead(up.status, { 'content-type': 'application/json' });
  rs.end(text);
}).listen(Number(port), '127.0.0.1');
console.log(`recording L1 requests on :${port} -> ${upstream} into ${log}`);
