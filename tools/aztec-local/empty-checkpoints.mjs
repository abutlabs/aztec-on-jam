// Prove only what was used. Aztec's sequencer can build a checkpoint every slot, empty or not, and
// every checkpoint costs Aztec's prover its share of an epoch proof: on a laptop the chain then
// outruns the prover (an epoch of four 36 s slots took one agent 6.5 to 9 minutes to prove), and
// the load delays JAM's confirmations until Aztec abandons checkpoints. So empty checkpoints are
// off, through the node's admin API (Aztec's own client), with two exceptions:
//   - block 1: a transaction built before it is valid only in block 1, so the sequencer starts with
//     empty checkpoints on and this turns them off once block 1 is in;
//   - a long idle: a transaction expires 24 h after its anchor block (MAX_TX_LIFETIME), so a chain
//     with no block for that long would never take a transaction again. After IDLE_HOURS (default
//     12) with no block, this turns empty checkpoints on for one block.
//   node empty-checkpoints.mjs <node url> <admin url>[,<admin url>…] <admin api key>
import './node-check.mjs';
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { createAztecNodeAdminClient } from '@aztec-labs/stdlib/interfaces/client';

const [nodeUrl, adminUrls, key] = process.argv.slice(2);
if (!key) { console.error('usage: node empty-checkpoints.mjs <node url> <admin url>[,…] <admin api key>'); process.exit(2); }
const IDLE_S = Number(process.env.IDLE_HOURS ?? 12) * 3600;
const node = createAztecNodeClient(nodeUrl);
// every sequencer: in a committee each proposes in its own slots
const admins = adminUrls.split(',').map((u) => createAztecNodeAdminClient(u, {}, undefined, key));
const say = (s) => console.log(`[${new Date().toISOString().slice(11, 19)}] ${s}`);
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const setEmpty = async (on) => { for (const a of admins) await a.setConfig({ buildCheckpointIfEmpty: on }); };
const head = async () => {
  const n = await node.getBlockNumber();
  return { n, ts: Number((await node.getBlockData(n)).header.globalVariables.timestamp) };
};

while ((await head().catch(() => ({ n: 0 }))).n < 1) await pause(3000);
await setEmpty(false);
say('block 1 is in: empty checkpoints off, so only blocks with transactions are built and proved');

for (;;) {
  await pause(60_000);
  try {
    const { n, ts } = await head();
    if (Date.now() / 1000 - ts < IDLE_S) continue;
    say(`no block for ${((Date.now() / 1000 - ts) / 3600).toFixed(1)} h: one empty checkpoint, so new transactions do not anchor to a block about to expire`);
    await setEmpty(true);
    const t0 = Date.now();
    while ((await node.getBlockNumber()) <= n && Date.now() - t0 < 600_000) await pause(5000);
    await setEmpty(false);
    say(`block ${await node.getBlockNumber()} is in; empty checkpoints off again`);
  } catch (e) {
    say(`could not check the chain: ${e.message ?? e}`);   // the next minute tries again
  }
}
