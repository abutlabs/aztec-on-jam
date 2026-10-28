// The Imperial Ledger: what this testnet shows anyone, and what an account's viewing keys add.
// Laid out as a block explorer: a search for an address, a transaction hash or a block, the chain at
// a glance, the newest blocks and transactions, and a page for each block, transaction and address.
// Public, from three sources: Aztec's node (each block's transactions as Aztec publishes them:
// commitments, nullifiers, ciphertext, public writes; never a sender, recipient or amount, and a
// Token's public storage), the JAM sidecar's jam_activity (each checkpoint's and epoch proof's
// work package), and a JAM validator's JIP-2 RPC (JAM's own chain). Private: a viewing-keys bundle
// pasted from the wallet is registered in this tab's own PXE (in memory, no prover, no signing
// key), which finds and decrypts that account's notes. Nothing here can spend.
//   ?node=  Aztec's node (default http://localhost:8080)   ?jam=  the sidecar (default :8545)
//   ?jip2=  a JAM validator's JIP-2 RPC (default ws://localhost:42950)   ?token=  the token shown
// Pages are routes: #/ home, #/block/<number or hash>, #/tx/<hash>, #/address/<address>, #/jam, #/keys.
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { AztecAddress } from '@aztec-labs/aztec.js/addresses';
import { TxHash } from '@aztec-labs/aztec.js/tx';
import { BlockHash } from '@aztec-labs/stdlib/block';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';
import { parseViewKeys, importViewKeys } from './view-keys.js';
import { publicBalance, totalSupply } from './public-state.js';

const $ = (id) => document.getElementById(id);
const params = new URLSearchParams(location.search);
const NODE_URL = params.get('node') ?? 'http://localhost:8080';
const JAM_URL = params.get('jam') ?? 'http://localhost:8545';
const JIP2_URL = params.get('jip2') ?? 'ws://localhost:42950';
const SHOWN = 60;        // blocks kept in memory, for the latest transactions and the "yours" marks
const LATEST = 10;       // rows in each home list
const HASH = /^0x[0-9a-fA-F]{64}$/;
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
/** a link to one of this page's routes */
function link(route, text, cls = 'mono') {
  const a = el('a', cls, text);
  a.href = `#/${route}`;
  return a;
}
function ago(seconds) {
  const s = Math.max(0, Math.round(Date.now() / 1000 - Number(seconds)));
  if (s < 60) return `${s} s ago`;
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

let node, tips, token;   // token: the AztecAddress shown in balances and the private view

// --- where a block stands: built by the sequencer, recorded on JAM (its checkpoint), proven on JAM
const MEANS = {
  proposed: 'built by Aztec’s sequencer; its checkpoint is not on JAM yet',
  settled: 'JAM checked its data and the proposer’s signatures and holds it in order; not yet proven valid',
  proven: 'JAM verified Aztec’s epoch proof covering this block: its transactions are proven valid',
};
function stage(n) {
  if (tips && n <= Number(tips.proven.block.number)) return ['proven', 'proven on JAM'];
  if (tips && n <= Number(tips.checkpointed.block.number)) return ['settled', 'recorded on JAM'];
  return ['proposed', 'proposed'];
}
function chip(n) {
  const [cls, text] = stage(n);
  const c = el('span', `chip ${cls}`, text);
  c.title = MEANS[cls];
  c.dataset.block = String(n);
  return c;
}
/** refreshes every stage chip on the page, as blocks move from proposed to recorded to proven */
function restage() {
  for (const c of document.querySelectorAll('.chip[data-block]')) {
    const [cls, text] = stage(Number(c.dataset.block));
    if (c.textContent !== text) { c.className = `chip ${cls}`; c.textContent = text; c.title = MEANS[cls]; }
  }
}

// --- routes --------------------------------------------------------------------------------------
function show(page) {
  for (const p of document.querySelectorAll('[data-panel]')) p.hidden = p.dataset.panel !== page;
  for (const t of document.querySelectorAll('[role=tab]')) t.setAttribute('aria-selected', String(t.dataset.tab === page));
}
for (const t of document.querySelectorAll('[role=tab]')) {
  t.onclick = () => { location.hash = t.dataset.tab === 'home' ? '#/' : `#/${t.dataset.tab}`; };
}
async function route() {
  const [, page = '', arg = ''] = location.hash.match(/^#\/([a-z]*)\/?(.*)$/) ?? [];
  $('found').textContent = '';
  try {
    if (page === 'block') { show('block'); await blockPage(decodeURIComponent(arg)); }
    else if (page === 'tx') { show('tx'); await txPage(decodeURIComponent(arg)); }
    else if (page === 'address') { show('address'); await addressPage(decodeURIComponent(arg)); }
    else if (page === 'jam' || page === 'keys') show(page);
    else show('home');
    window.scrollTo(0, 0);
  } catch (e) { console.error(e); $('found').textContent = `${e.message ?? e}`; }
}
window.addEventListener('hashchange', route);

// --- search: a number is a block; a 32-byte hex is a transaction, a block, or else an address ---
$('search').onsubmit = (ev) => {
  ev.preventDefault();
  const q = $('q').value.trim();
  act('Search', async () => {
    if (/^\d+$/.test(q)) { location.hash = `#/block/${q}`; return; }
    if (!HASH.test(q)) throw new Error('enter an address, a transaction hash (0x and 64 hex digits) or a block number');
    if (await node.getTxEffect(TxHash.fromString(q))) { location.hash = `#/tx/${q}`; return; }
    if (await node.getBlock(BlockHash.fromString(q)).catch(() => undefined)) { location.hash = `#/block/${q}`; return; }
    location.hash = `#/address/${q}`;
  });
};

// --- detail pages --------------------------------------------------------------------------------
/** a two-column list of facts: [label, value (text or element), title?] */
function facts(rows) {
  const dl = el('dl', 'kvlist');
  for (const [k, v, title] of rows) {
    const dd = el('dd', null, null, v instanceof Node ? v : document.createTextNode(String(v)));
    if (title) dd.title = title;
    dl.append(el('dt', null, k), dd);
  }
  return dl;
}
function valueList(values, empty = 'none') {
  if (!values.length) return el('span', 'muted', empty);
  return el('ol', 'values mono', null, ...values.map((v) => el('li', null, String(v))));
}

async function blockPage(arg) {
  const b = await node.getBlock(HASH.test(arg) ? BlockHash.fromString(arg) : Number(arg), { includeTransactions: true });
  if (!b) throw new Error(`no block ${arg} on Aztec's node`);
  const n = Number(b.number), g = b.header.globalVariables, txs = b.body.txEffects;
  const nav = el('div', 'pager', null, n > 1 ? link(`block/${n - 1}`, `← block ${n - 1}`, '') : el('span'),
    n < Number(tips.proposed.number) ? link(`block/${n + 1}`, `block ${n + 1} →`, '') : el('span'));
  $('block-view').replaceChildren(
    el('div', 'cardhead', null, el('h2', null, `Block ${n}`), chip(n)),
    facts([
      ['Status', chip(n)],
      ['Time', `${new Date(Number(g.timestamp) * 1000).toLocaleString()} (${ago(g.timestamp)})`],
      ['Transactions', `${txs.length}`],
      ['Checkpoint', `${b.checkpointNumber} (block ${Number(b.indexWithinCheckpoint) + 1} of it)`, 'blocks are grouped into checkpoints; each checkpoint is one work package on JAM'],
      ['Slot', String(g.slotNumber)],
      ['Block hash', el('span', 'mono wrap', String(b.hash))],
      ['Archive root', el('span', 'mono wrap', String(b.archive.root)), 'the root of the tree of all block hashes up to and including this one'],
      ['Fees', `${g.gasFees.feePerDaGas} per DA gas · ${g.gasFees.feePerL2Gas} per L2 gas`],
      ['Coinbase', el('span', 'mono wrap', String(g.coinbase))],
      ['Fee recipient', link(`address/${g.feeRecipient}`, String(g.feeRecipient), 'mono wrap')],
    ]),
    el('h3', null, 'Transactions'),
    txs.length ? el('ol', 'items', null, ...txs.map((t) => txItem(t, n, g.timestamp))) : el('p', 'muted', 'an empty block'),
    nav);
  markYours();
}

async function txPage(arg) {
  if (!HASH.test(arg)) throw new Error(`not a transaction hash: ${arg}`);
  const found = await node.getTxEffect(TxHash.fromString(arg));
  if (!found) throw new Error(`no transaction ${short(arg)} on Aztec's node`);
  const t = found.data, n = Number(found.l2BlockNumber);
  const logs = t.privateLogs;
  const box = el('div', 'tx', null);
  box.dataset.tx = t.txHash.toString();
  box.append(
    el('div', 'cardhead', null, el('h2', null, 'Transaction'), el('span', 'yours')),
    facts([
      ['Hash', el('span', 'mono wrap', t.txHash.toString())],
      ['Status', chip(n)],
      ['Block', link(`block/${n}`, `${n}`)],
      ['Position', `transaction ${found.txIndexInBlock + 1} in the block`],
      ['Result', t.revertCode.isOK() ? 'succeeded' : `reverted (${t.revertCode})`],
      ['Fee', `${t.transactionFee.toBigInt()}`, 'in fee juice, paid here by the network’s sponsor'],
      ['Sender, recipient, amount', el('span', 'muted', 'not published: a private payment shows none of them')],
    ]),
    el('h3', null, `Note hashes (${t.noteHashes.length})`), el('p', 'note', 'commitments to new private notes'), valueList(t.noteHashes),
    el('h3', null, `Nullifiers (${t.nullifiers.length})`), el('p', 'note', 'marks of spent notes, and of the transaction itself; not linkable to the notes'), valueList(t.nullifiers),
    el('h3', null, `Public writes (${t.publicDataWrites.length})`), el('p', 'note', 'public state changed in the open, fees included'),
    valueList(t.publicDataWrites.map((w) => `slot ${short(w.leafSlot)} ← ${w.value}`)),
    el('h3', null, `Encrypted logs (${logs.length})`), el('p', 'note', 'ciphertext: only the recipient’s keys open it'),
    logs[0] ? el('details', 'raw', null, el('summary', null, 'what an encrypted log looks like'),
      el('pre', 'mono', logs[0].fields.slice(0, logs[0].emittedLength ?? logs[0].fields.length).map(String).join('\n'))) : el('span', 'muted', 'none'),
    el('h3', null, `Public logs (${t.publicLogs.length})`), el('p', 'note', 'events published in the open'),
  );
  $('tx-view').replaceChildren(box);
  markYours();
}

async function addressPage(arg) {
  const who = AztecAddress.fromStringUnsafe(arg);
  const contract = await node.getContract(who);
  const rows = [
    ['Address', el('span', 'mono wrap', who.toString())],
    ['Kind', contract ? 'a contract' : 'an account, or an address with no contract on this chain', contract ? `class ${contract.currentContractClassId}` : undefined],
  ];
  if (contract) rows.push(['Contract class', el('span', 'mono wrap', String(contract.currentContractClassId))], ['Deployer', link(`address/${contract.deployer}`, String(contract.deployer), 'mono wrap')]);
  if (token && who.equals(token)) rows.push(['Total supply', `${await totalSupply(node, token)} RBL, public and private together`]);
  if (token) rows.push(['Public balance', `${await publicBalance(node, token, who)} RBL`, `read straight from the token's public storage (${token})`]);
  const isViewed = viewed && who.equals(viewed);
  rows.push(['Private balance', isViewed ? `${$('priv').textContent} RBL, found with this account's viewing keys`
    : el('span', null, null, document.createTextNode('only with this account’s viewing keys: '), link('keys', 'paste them', ''))]);
  $('address-view').replaceChildren(
    el('div', 'cardhead', null, el('h2', null, contract ? 'Contract' : 'Address'), token ? el('span', 'muted small', `token ${short(token)}`) : el('span')),
    facts(rows),
    token ? el('span') : el('p', 'note', 'Choose a token (Viewing keys → Token) to read this address’s public balance.'),
    isViewed ? el('div', null, null, el('h3', null, 'Payments received'), $('history').cloneNode(true)) : el('span'));
}

// --- home: the latest blocks and transactions ----------------------------------------------------
const blocks = new Map();   // block number -> the block, with its transactions
let newest = 0;

function txItem(t, n, timestamp) {
  const li = el('li', 'item tx', null,
    el('span', 'icon', 'Tx'),
    el('div', 'main', null, link(`tx/${t.txHash}`, short(t.txHash)), el('small', 'muted', `block ${n} · ${ago(timestamp)}`)),
    el('div', 'mid small', `${t.noteHashes.length} note hashes · ${t.nullifiers.length} nullifiers · ${t.privateLogs.length} encrypted logs`),
    el('span', 'yours'));
  li.dataset.tx = t.txHash.toString();
  return li;
}

function blockItem(b) {
  const n = Number(b.number), txs = b.body.txEffects;
  const li = el('li', txs.length ? 'item block' : 'item block empty', null,
    el('span', 'icon', 'Bk'),
    el('div', 'main', null, link(`block/${n}`, `Block ${n}`), el('small', 'muted', ago(b.header.globalVariables.timestamp))),
    el('div', 'mid small', `${txs.length} tx${txs.length === 1 ? '' : 's'} · checkpoint ${b.checkpointNumber}`),
    chip(n));
  li.dataset.n = String(n);
  return li;
}

function renderLatest() {
  const ordered = [...blocks.values()].sort((a, b) => Number(b.number) - Number(a.number));
  $('blocks').replaceChildren(...ordered.slice(0, $('show-empty').checked ? LATEST : SHOWN)
    .filter((b) => $('show-empty').checked || b.body.txEffects.length).slice(0, LATEST).map(blockItem));
  const txs = ordered.flatMap((b) => b.body.txEffects.map((t) => txItem(t, Number(b.number), b.header.globalVariables.timestamp)));
  $('txs').replaceChildren(...(txs.length ? txs.slice(0, LATEST) : [el('li', 'muted', 'no transactions yet')]));
  if (!$('blocks').children.length) $('blocks').append(el('li', 'muted', 'only empty blocks so far'));
  markYours();
}

async function pollBlocks() {
  tips = await node.getChainTips();
  const top = Number(tips.proposed.number);
  if (top < newest) { blocks.clear(); newest = 0; }   // the proposed chain was pruned
  let added = false;
  for (let from = Math.max(newest + 1, top - SHOWN + 1, 1); from <= top; from += 25) {
    for (const b of await node.getBlocks(from, Math.min(25, top - from + 1), { includeTransactions: true })) { blocks.set(Number(b.number), b); added = true; }
  }
  newest = top;
  for (const n of [...blocks.keys()].sort((a, b) => a - b).slice(0, Math.max(0, blocks.size - SHOWN))) blocks.delete(n);
  if (added) renderLatest();
  restage();
  const latest = blocks.get(top);
  $('s-block').replaceChildren(link(`block/${top}`, String(top)));
  $('s-block-age').textContent = latest ? ago(latest.header.globalVariables.timestamp) : '';
  $('s-settled').textContent = `block ${tips.checkpointed.block.number}`;
  $('s-settled-cp').textContent = `checkpoint ${tips.checkpointed.checkpoint.number}`;
  $('s-proven').textContent = `block ${tips.proven.block.number}`;
  $('s-proven-cp').textContent = `checkpoint ${tips.proven.checkpoint.number}`;
  $('net').textContent = `Aztec block ${top} · recorded on JAM ${tips.checkpointed.block.number} · proven ${tips.proven.block.number}`;
}
$('show-empty').onchange = renderLatest;

// --- what JAM holds: checkpoints and epoch proofs ------------------------------------------------
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
/** a work package in one short line; its hash in the tooltip */
function packageLine(r) {
  const e = el('div', 'mid small', `work package ${kb(r.bytes)} · core ${r.core} · anchored at JAM slot ${r.anchor_slot}`);
  e.title = `work package ${r.package_hash}`;
  return e;
}

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

  const epochs = activity.filter((e) => 'relayingEpochProof' in e).reverse().slice(0, LATEST).map((e) => {
    const s = sent.get(e.relayingEpochProof), rc = receipt(s);
    const [cls, text] = !s ? ['proposed', 'handing to JAM'] : s.code !== 0 ? ['bad', 'JAM refused it']
      : provenCp >= Number(e.end) ? ['proven', 'verified on JAM'] : ['settled', 'JAM is verifying'];
    return el('li', 'item', null, el('span', 'icon', 'Ep'),
      el('div', 'main', null, el('b', null, `Checkpoints ${e.start}–${e.end}`), el('small', 'muted', `proof ${kb(e.proofBytes)}`)),
      rc ? packageLine(rc) : el('div', 'mid'), el('span', `chip ${cls}`, text));
  });
  $('epochs').replaceChildren(...(epochs.length ? epochs : [el('li', 'muted', 'none yet since the sidecar started: Aztec’s prover proves an epoch at a time')]));

  const cps = last >= 1 ? await node.getCheckpoints(Math.max(1, last - LATEST + 1), Math.min(LATEST, last)) : [];
  $('checkpoints').replaceChildren(...cps.reverse().map((c) => {
    const relay = relayBySlot.get(String(c.header.slotNumber)), rc = receipt(sent.get(relay?.relaying));
    const end = Number(c.startBlock) + c.blockCount - 1;
    const blocksOf = c.blockCount > 1 ? el('span', null, null, link(`block/${c.startBlock}`, `blocks ${c.startBlock}`, ''), document.createTextNode(`–${end}`))
      : link(`block/${c.startBlock}`, `block ${c.startBlock}`, '');
    const [cls, text] = Number(c.number) <= provenCp ? ['proven', 'proven on JAM'] : ['settled', 'recorded on JAM'];
    return el('li', 'item', null, el('span', 'icon', 'Cp'),
      el('div', 'main', null, el('b', null, `Checkpoint ${c.number}`), el('small', 'muted', null, document.createTextNode('Aztec '), blocksOf, document.createTextNode(` · slot ${c.header.slotNumber}`))),
      rc ? packageLine(rc) : el('div', 'mid small', 'relayed before the sidecar’s current run'),
      el('span', `chip ${cls}`, text));
  }));
}

// --- the token: its supply on the home page, balances on address pages -------------------------
async function setToken(address) {
  token = address;
  $('token').value = address.toString();
  await showSupply();
  await refreshPrivate();
}
async function showSupply() {
  if (!token) return;
  $('supply').textContent = `${await totalSupply(node, token)} RBL`;
  $('supply-of').replaceChildren(document.createTextNode('public and private together · '), link(`address/${token}`, short(token)));
}
$('token').onchange = () => act('Read the token', () => setToken(AztecAddress.fromStringUnsafe($('token').value.trim())));

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
  $('s-jam').textContent = `slot ${fin.slot}`;
  $('s-jam-best').textContent = `best slot ${best.slot}`;
  if ($('jam-blocks').closest('[data-panel]').hidden) return;   // the walk back costs ten calls; only while shown
  const rows = [];
  for (let b = best, i = 0; b && i < 10; i++, b = await jip2('parent', [b.header_hash]).catch(() => null)) {
    const done = b.slot <= fin.slot;
    rows.push(el('li', 'item', null, el('span', 'icon', 'J'), el('div', 'main', null, el('b', null, `slot ${b.slot}`), el('small', 'muted mono', short(hex(b.header_hash)))),
      el('span'), el('span', `chip ${done ? 'proven' : 'proposed'}`, done ? 'finalized' : 'not final yet')));
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
    const y = box.querySelector('.yours');
    if (y) y.textContent = what ? `yours: ${what}` : '';
  }
}

$('unlock').onclick = () => act('Read with viewing keys', async () => {
  const bundle = parseViewKeys($('keys').value);
  say('working: opening a PXE in this tab (in memory, no prover)');
  viewer ??= await EmbeddedWallet.create(NODE_URL, { pxe: { proverEnabled: false }, ephemeral: true });
  viewed = await importViewKeys(viewer, bundle);
  $('keys').value = '';   // the keys now live only in the PXE's memory
  $('who').textContent = viewed.toString();
  $('who').href = `#/address/${viewed}`;
  $('locked').hidden = true; $('unlocked').hidden = false;
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
      return el('li', 'item', null, el('span', 'icon', 'Tx'), el('div', 'main', null, el('b', null, what), link(`tx/${m.txHash}`, `tx ${short(m.txHash)}`, 'mono small')),
        el('span'), link(`block/${m.l2BlockNumber}`, `block ${m.l2BlockNumber}`, 'small'));
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
  catch (e) { console.error(e); say(`error: ${what}: ${e.message ?? e}`, 'error'); $('found').textContent = e.message ?? String(e); }
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
  await route();
  every(4000, 'Aztec blocks', pollBlocks);
  every(5000, 'JAM settlement', pollSettlement);
  every(6000, 'JAM chain', pollJam);
  every(10000, 'the private view', refreshPrivate);
  say('ready', 'done');
  document.body.dataset.ready = '1';
}
boot().catch((e) => { console.error(e); say(`error: ${e.message ?? e}`, 'error'); });
