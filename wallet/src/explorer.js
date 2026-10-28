// The Imperial Ledger: what this testnet shows anyone, and what an account's viewing keys add.
// Public, from three sources: Aztec's node (each block's transactions as Aztec publishes them:
// commitments, nullifiers, ciphertext, public writes; never a sender, recipient or amount, and a
// Token's public storage), the JAM sidecar's jam_activity (each checkpoint's and epoch proof's
// work package), and a JAM validator's JIP-2 RPC (JAM's own chain). Private: a viewing-keys bundle
// pasted from the wallet is registered in this tab's own PXE (in memory, no prover, no signing
// key), which finds and decrypts that account's notes. Nothing here can spend.
//   ?node=  Aztec's node (default http://localhost:8080)   ?jam=  the sidecar (default :8545)
//   ?jip2=  a JAM validator's JIP-2 RPC (default ws://localhost:42950)   ?token=  the token shown
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { AztecAddress } from '@aztec-labs/aztec.js/addresses';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';
import { parseViewKeys, importViewKeys } from './view-keys.js';
import { publicBalance, totalSupply } from './public-state.js';

const $ = (id) => document.getElementById(id);
const params = new URLSearchParams(location.search);
const NODE_URL = params.get('node') ?? 'http://localhost:8080';
const JAM_URL = params.get('jam') ?? 'http://localhost:8545';
const JIP2_URL = params.get('jip2') ?? 'ws://localhost:42950';
const SHOWN = 60;   // blocks kept on the page
const short = (a) => { const s = String(a); return `${s.slice(0, 10)}…${s.slice(-6)}`; };
const kb = (n) => (n < 1024 ? `${n} B` : `${(n / 1024).toFixed(1)} kB`);
const say = (text, cls = '') => { $('status').textContent = text; $('status').className = cls; };
/** an element with a class and text; children may follow */
function el(tag, cls, text, ...children) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined && text !== null) e.textContent = text;
  e.append(...children);
  return e;
}

let node, tips, token;   // token: the AztecAddress shown in balances and the private view

// --- tabs ----------------------------------------------------------------------------------------
for (const t of document.querySelectorAll('[role=tab]')) {
  t.onclick = () => {
    for (const u of document.querySelectorAll('[role=tab]')) u.setAttribute('aria-selected', String(u === t));
    for (const p of document.querySelectorAll('.public [data-panel]')) p.hidden = p.dataset.panel !== t.dataset.tab;
  };
}

// --- Aztec blocks --------------------------------------------------------------------------------
const blocks = new Map();   // block number -> its <li>
let newest = 0;

/** where a block stands: built by the sequencer, recorded on JAM (its checkpoint), proven on JAM */
function stage(n) {
  if (n <= Number(tips.proven.block.number)) return ['proven', 'proven on JAM'];
  if (n <= Number(tips.checkpointed.block.number)) return ['settled', 'recorded on JAM'];
  return ['proposed', 'proposed'];
}

function txView(t) {
  const logs = t.privateLogs;
  const facts = [
    ['note hashes', t.noteHashes.length, 'commitments to new private notes'],
    ['nullifiers', t.nullifiers.length, 'marks of spent notes (and the transaction itself), not linkable to the notes'],
    ['encrypted logs', logs.length, 'ciphertext: only the recipient’s keys open it'],
    ['public writes', t.publicDataWrites.length, 'public state changed in the open (fees included)'],
    ['public logs', t.publicLogs.length, 'events published in the open'],
    ['fee', t.transactionFee.toBigInt(), 'in fee juice, paid here by the network’s sponsor'],
  ];
  const dl = el('dl', 'facts');
  for (const [k, v, title] of facts) { const dd = el('dd', 'mono', String(v)); dd.title = title; dl.append(el('dt', null, k), dd); }
  const box = el('div', 'tx', null, el('div', 'txhead', null, el('span', 'mono', `tx ${short(t.txHash)}`), el('span', 'yours')), dl);
  box.dataset.tx = t.txHash.toString();
  if (logs[0]) {
    const fields = logs[0].fields.slice(0, logs[0].emittedLength ?? logs[0].fields.length);
    box.append(el('details', 'raw', null, el('summary', null, 'what an encrypted log looks like'), el('pre', 'mono', fields.map(String).join('\n'))));
  }
  return box;
}

function blockView(b) {
  const txs = b.body.txEffects;
  const summary = el('summary', null, null, el('b', null, `Block ${b.number}`), el('span', 'chip'),
    el('span', 'muted', `${txs.length} tx${txs.length === 1 ? '' : 's'} · checkpoint ${b.checkpointNumber}`));
  const d = el('details', null, null, summary, ...txs.map(txView));
  const li = el('li', txs.length ? 'block' : 'block empty', null, d);
  li.dataset.n = String(b.number);
  return li;
}

async function pollBlocks() {
  tips = await node.getChainTips();
  const top = Number(tips.proposed.number);
  if (top < newest) { $('blocks').replaceChildren(); blocks.clear(); newest = 0; }   // the proposed chain was pruned
  for (let from = Math.max(newest + 1, top - SHOWN + 1, 1); from <= top; from += 25) {
    for (const b of await node.getBlocks(from, Math.min(25, top - from + 1), { includeTransactions: true })) {
      const li = blockView(b);
      $('blocks').prepend(li);
      blocks.set(Number(b.number), li);
    }
  }
  newest = top;
  for (const n of [...blocks.keys()].sort((a, b) => a - b).slice(0, Math.max(0, blocks.size - SHOWN))) { blocks.get(n).remove(); blocks.delete(n); }
  for (const [n, li] of blocks) {
    const [cls, text] = stage(n);
    const chip = li.querySelector('.chip');
    if (chip.textContent !== text) { chip.className = `chip ${cls}`; chip.textContent = text; }
  }
  markYours();
  $('net').textContent = `Aztec block ${top} · recorded on JAM ${tips.checkpointed.block.number} · proven ${tips.proven.block.number}`;
}
$('show-empty').onchange = (e) => $('blocks').classList.toggle('all', e.target.checked);

// --- settled on JAM ------------------------------------------------------------------------------
const activity = [];
let since = 0, settleKey = '';

async function rpc(url, method, params = []) {
  const r = await fetch(url, { method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) }).then((x) => x.json());
  if (r.error) throw new Error(r.error.message);
  return r.result;
}
/** the work package's receipt the sidecar logged from jamboree, when it parses */
const receipt = (e) => { try { return JSON.parse(e?.out ?? ''); } catch { return null; } };
const packageLine = (r) => `work package ${short(r.package_hash)} · core ${r.core} · ${kb(r.bytes)} · anchored at JAM slot ${r.anchor_slot}`;

async function pollSettlement() {
  const r = await rpc(JAM_URL, 'jam_activity', [since]);
  for (const e of r.events) { since = Math.max(since, e.t); activity.push(e); }
  const last = Number(tips.checkpointed.checkpoint.number);
  const provenCp = Number(tips.proven.checkpoint.number);
  const key = `${activity.length}:${last}:${provenCp}`;
  if (key === settleKey) return;
  settleKey = key;

  const sent = new Map(activity.filter((e) => 'submitted' in e || 'submittedEpochProof' in e).map((e) => [e.submitted ?? e.submittedEpochProof, e]));
  const relayBySlot = new Map(activity.filter((e) => 'relaying' in e).map((e) => [e.slot, e]));

  const epochs = activity.filter((e) => 'relayingEpochProof' in e).reverse().map((e) => {
    const s = sent.get(e.relayingEpochProof), rc = receipt(s);
    const [cls, text] = !s ? ['proposed', 'handing to JAM'] : s.code !== 0 ? ['bad', 'JAM refused it']
      : provenCp >= Number(e.end) ? ['proven', 'verified on JAM'] : ['settled', 'JAM is verifying'];
    return el('li', 'row', null,
      el('div', 'rowhead', null, el('b', null, `Checkpoints ${e.start}–${e.end}`), el('span', `chip ${cls}`, text)),
      el('div', 'muted', `proof ${kb(e.proofBytes)}${rc ? ' · ' + packageLine(rc) : ''}`));
  });
  $('epochs').replaceChildren(...(epochs.length ? epochs : [el('li', 'muted', 'none yet since the sidecar started: Aztec’s prover proves an epoch at a time')]));

  const cps = last >= 1 ? await node.getCheckpoints(Math.max(1, last - 29), Math.min(30, last)) : [];
  $('checkpoints').replaceChildren(...cps.reverse().map((c) => {
    const relay = relayBySlot.get(String(c.header.slotNumber)), rc = receipt(sent.get(relay?.relaying));
    const blocksOf = c.blockCount > 1 ? `blocks ${c.startBlock}–${Number(c.startBlock) + c.blockCount - 1}` : `block ${c.startBlock}`;
    const [cls, text] = Number(c.number) <= provenCp ? ['proven', 'proven on JAM'] : ['settled', 'recorded on JAM'];
    return el('li', 'row', null,
      el('div', 'rowhead', null, el('b', null, `Checkpoint ${c.number}`), el('span', `chip ${cls}`, text)),
      el('div', 'muted', `Aztec ${blocksOf} · slot ${c.header.slotNumber}`),
      el('div', 'muted mono small', rc ? packageLine(rc) : 'relayed before the sidecar’s current run'));
  }));
}

// --- public balances -----------------------------------------------------------------------------
async function setToken(address) {
  token = address;
  $('token').value = address.toString();
  $('looked').textContent = '';
  await showSupply();
  await refreshPrivate();
}
async function showSupply() {
  if (!token) { $('supply').textContent = ''; return; }
  $('supply').textContent = `Total supply: ${await totalSupply(node, token)} RBL, public and private together`;
}
$('token').onchange = () => act('Read the token', () => setToken(AztecAddress.fromStringUnsafe($('token').value.trim())));
$('look').onclick = () => act('Read a public balance', async () => {
  if (!token) throw new Error('choose a token first');
  const who = AztecAddress.fromStringUnsafe($('lookup').value.trim());
  $('looked').textContent = `${short(who)} holds ${await publicBalance(node, token, who)} RBL publicly`;
});

// --- JAM chain (JIP-2) ---------------------------------------------------------------------------
let ws, wsReady, wsId = 0;
const pending = new Map();
function jip2(method, params = []) {
  if (!ws || ws.readyState > 1) {
    ws = new WebSocket(JIP2_URL);
    wsReady = new Promise((ok, fail) => { ws.onopen = ok; ws.onerror = () => fail(new Error(`JIP-2 not reachable at ${JIP2_URL}`)); });
    ws.onmessage = (m) => { const o = JSON.parse(m.data); pending.get(o.id)?.(o); pending.delete(o.id); };
  }
  return wsReady.then(() => new Promise((ok, fail) => {
    const id = ++wsId;
    pending.set(id, (o) => (o.error ? fail(new Error(o.error.message)) : ok(o.result)));
    ws.send(JSON.stringify({ jsonrpc: '2.0', id, method, params }));
  }));
}
const hex = (b64) => `0x${[...atob(b64)].map((c) => c.charCodeAt(0).toString(16).padStart(2, '0')).join('')}`;

async function pollJam() {
  const [best, fin] = await Promise.all([jip2('bestBlock'), jip2('finalizedBlock')]);
  $('jam-best').textContent = `slot ${best.slot}`;
  $('jam-final').textContent = `slot ${fin.slot}`;
  const rows = [];
  for (let b = best, i = 0; b && i < 10; i++, b = await jip2('parent', [b.header_hash]).catch(() => null)) {
    const done = b.slot <= fin.slot;
    rows.push(el('li', 'row', null, el('div', 'rowhead', null, el('span', null, `slot ${b.slot}`),
      el('span', `chip ${done ? 'proven' : 'proposed'}`, done ? 'finalized' : 'not final yet')), el('div', 'muted small', short(hex(b.header_hash)))));
  }
  $('jam-blocks').replaceChildren(...rows);
}

// --- the private view ----------------------------------------------------------------------------
let viewer, viewed;
const registered = new Set();   // tokens registered in the viewer's PXE
const yours = new Map();        // tx hash -> what these keys say it was

function markYours() {
  for (const box of document.querySelectorAll('.tx[data-tx]')) {
    const what = yours.get(box.dataset.tx);
    box.querySelector('.yours').textContent = what ? `yours: ${what}` : '';
    box.closest('.block')?.classList.toggle('mine', [...box.closest('.block').querySelectorAll('.tx')].some((t) => yours.has(t.dataset.tx)));
  }
}

$('unlock').onclick = () => act('Read with viewing keys', async () => {
  const bundle = parseViewKeys($('keys').value);
  say('working: opening a PXE in this tab (in memory, no prover)');
  viewer ??= await EmbeddedWallet.create(NODE_URL, { pxe: { proverEnabled: false }, ephemeral: true });
  viewed = await importViewKeys(viewer, bundle);
  $('keys').value = '';   // the keys now live only in the PXE's memory
  $('who').textContent = viewed.toString();
  $('locked').hidden = true; $('unlocked').hidden = false;
  if (!$('lookup').value) $('lookup').value = viewed.toString();
  if (bundle.token && !token) await setToken(bundle.token); else await refreshPrivate();
});
$('forget').onclick = () => location.reload();

let reading = false;
async function refreshPrivate() {
  if (!viewed || !token || reading) return;
  reading = true;
  try {
    if (!registered.has(token.toString())) {
      const instance = await node.getContract(token);
      if (!instance) throw new Error(`the node has no contract at ${token}`);
      await viewer.registerContract(instance, TokenContract.artifact);
      registered.add(token.toString());
    }
    const c = await TokenContract.at(token, viewer);
    // a simulation syncs the PXE to the chain's head first: notes are found, spends counted
    const [priv, pub] = await Promise.all([c.methods.balance_of_private(viewed).simulate({ from: viewed }), publicBalance(node, token, viewed)]);
    $('priv').textContent = String(priv.result);
    $('pub').textContent = String(pub);
    const events = await viewer.getPrivateEvents(TokenContract.events.Transfer, { contractAddress: token, scopes: [viewed] });
    yours.clear();
    const me = viewed.toString();
    const rows = events.slice().reverse().map(({ event: e, metadata: m }) => {
      const from = String(e.from), to = String(e.to);
      const what = to === me && from === me ? `${e.amount} to yourself` : to === me ? `received ${e.amount} from ${short(from)}` : `sent ${e.amount} to ${short(to)}`;
      yours.set(m.txHash.toString(), what);
      return el('li', 'row', null, el('div', 'rowhead', null, el('b', null, what), el('span', 'muted small', `block ${m.l2BlockNumber}`)),
        el('div', 'muted mono small', `tx ${short(m.txHash)}`));
    });
    $('history').replaceChildren(...(rows.length ? rows : [el('li', 'muted', 'no payments received yet')]));
    markYours();
  } finally { reading = false; }
}

// --- one action at a time, reported on the status line -----------------------------------------
async function act(what, body) {
  for (const b of document.querySelectorAll('button:not([role=tab])')) b.disabled = true;
  say(`working: ${what}`);
  try { await body(); say(`${what} — done`, 'done'); }
  catch (e) { console.error(e); say(`error: ${what}: ${e.message ?? e}`, 'error'); }
  finally { for (const b of document.querySelectorAll('button')) b.disabled = false; }
}

// --- boot and polling ----------------------------------------------------------------------------
function every(ms, what, f) {
  const tick = async () => { await f().catch((e) => { console.error(what, e); say(`${what}: ${e.message ?? e}`, 'error'); }); setTimeout(tick, ms); };
  tick();
}

async function boot() {
  node = createAztecNodeClient(NODE_URL);
  await pollBlocks();
  $('grafana').href = 'http://localhost:3300/d/obs-chain/chain-health';
  if (params.get('token')) await setToken(AztecAddress.fromStringUnsafe(params.get('token')));
  every(4000, 'Aztec blocks', pollBlocks);
  every(5000, 'JAM settlement', pollSettlement);
  every(6000, 'JAM chain', pollJam);
  every(10000, 'the private view', refreshPrivate);
  say('ready', 'done');
  document.body.dataset.ready = '1';
}
boot().catch((e) => { console.error(e); say(`error: ${e.message ?? e}`, 'error'); });
