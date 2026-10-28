// Our wallet, on Aztec's released libraries. Everything cryptographic is Aztec's: EmbeddedWallet
// (browser build: the PXE in this tab, SQLite on OPFS), Aztec's prover (bb.js, threads — the
// page must be cross-origin isolated), Aztec's Token contract, and Aztec's note tagging for
// discovery. Fees go through the network's SponsoredFPC, funded at genesis, so a fresh account
// can act with no funds.
//
// The right-hand panel shows who is doing what, live, from three sources: this tab's own steps
// (the node client's sendTx is wrapped to mark where proving ends), Aztec's node (getChainTips:
// proposed, checkpointed, proven), and the JAM sidecar's jam_activity (relay milestones, JAM's
// finalized slot, the prover node's epoch jobs). Nothing here changes what Aztec's code does.
//   ?node=  Aztec's node (default http://localhost:8080)   ?jam=  the sidecar (default :8545)
//   ?explorer=  the explorer (default http://localhost:19730), given node, jam, jip2 and the token
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { getContractInstanceFromInstantiationParams } from '@aztec-labs/aztec.js/contracts';
import { AztecAddress } from '@aztec-labs/aztec.js/addresses';
import { SponsoredFeePaymentMethod } from '@aztec-labs/aztec.js/fee';
import { Fr, Fq } from '@aztec-labs/aztec.js/fields';
import { SPONSORED_FPC_SALT } from '@aztec-labs/constants';
import { SponsoredFPCContract } from '@aztec-labs/noir-contracts.js/SponsoredFPC';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';
import { exportViewKeys } from './view-keys.js';

const $ = (id) => document.getElementById(id);
const params = new URLSearchParams(location.search);
const NODE_URL = params.get('node') ?? 'http://localhost:8080';
const JAM_URL = params.get('jam') ?? 'http://localhost:8545';
const EXPLORER_URL = params.get('explorer') ?? 'http://localhost:19730';
const wait = { timeout: 600, interval: 2 };
const dur = (ms) => (ms < 60_000 ? `${(ms / 1000).toFixed(1)} s` : `${Math.floor(ms / 60_000)} min ${Math.round((ms % 60_000) / 1000)} s`);
const short = (a) => `${String(a).slice(0, 8)}…${String(a).slice(-6)}`;

let node, wallet, me, token, fpcAddress;

// --- the status line (e2e.mjs reads it: "working: …", "… — done in N s", "error: …") ------------
const say = (text, cls = '') => { $('status').textContent = text; $('status').className = cls; };

// --- busy timers: anything with data-t0 and state busy shows its elapsed time -----------------
setInterval(() => {
  for (const el of document.querySelectorAll('[data-state="busy"][data-t0]')) {
    el.querySelector(':scope > time, :scope > em').textContent = dur(Date.now() - Number(el.dataset.t0));
  }
}, 250);

// --- the five lanes ------------------------------------------------------------------------------
/** state: idle | busy | done | bad. A busy lane keeps its start time while it stays busy. */
function lane(id, text, state = 'idle', t0) {
  const el = $(`lane-${id}`);
  if (state === 'busy') { if (el.dataset.state !== 'busy' || t0) el.dataset.t0 = String(t0 ?? Date.now()); }
  else { delete el.dataset.t0; el.querySelector('time').textContent = ''; }
  el.dataset.state = state; el.dataset.at = String(Date.now());
  el.querySelector('.st').textContent = text;
}
/** a lane that may show background news: idle, recovered, or done more than 10 s ago */
const quiet = (id) => {
  const { state, at } = $(`lane-${id}`).dataset;
  return state === 'idle' || state === 'bad' || (state === 'done' && Date.now() - Number(at) > 10_000);
};

// --- the activity timeline: one entry per action, one step per stage ---------------------------
function activity(title) {
  const li = document.createElement('li');
  li.innerHTML = '<div class="head"><b></b><time></time></div><ol class="steps"></ol>';
  li.querySelector('b').textContent = title;
  li.querySelector('time').textContent = new Date().toLocaleTimeString();
  $('log').prepend(li);
  let open;
  return {
    begin(text) {
      this.end();
      const s = document.createElement('li');
      s.innerHTML = '<span></span><em></em>';
      s.firstChild.textContent = text; s.dataset.state = 'busy'; s.dataset.t0 = String(Date.now());
      li.querySelector('.steps').append(s); open = s;
    },
    /** closes the open step, optionally renaming it; returns its duration in ms */
    end(text, state = 'done') {
      if (!open) return 0;
      const ms = Date.now() - Number(open.dataset.t0);
      if (text) open.firstChild.textContent = text;
      open.dataset.state = state; delete open.dataset.t0; open.lastChild.textContent = dur(ms);
      open = undefined;
      return ms;
    },
  };
}

// --- this tab's part of a transaction: proving ends where the node client's sendTx begins -------
let current;   // the activity of the transaction in flight
function instrument(w) {
  const client = w.aztecNode;
  const sendTx = client?.sendTx?.bind(client);
  if (!sendTx) return;
  client.sendTx = async (tx) => {
    const ms = current?.end('Proved in this tab');
    lane('tab', `proved in ${dur(ms ?? 0)}`, 'done');
    current?.begin('Aztec node checks the proof');
    lane('node', 'checking your proof', 'busy');
    const out = await sendTx(tx);
    current?.end('Aztec node verified the proof and accepted it');
    current?.begin('Waiting for a block');
    lane('node', 'building a block with your transaction', 'busy');
    return out;
  };
}

/** Every action runs through here: one at a time, buttons locked, the timeline and lanes told. */
async function run(what, body, { tx = false } = {}) {
  for (const b of document.querySelectorAll('button:not([role=tab])')) b.disabled = true;
  say(`working: ${what}`);
  const a = activity(what);
  const t0 = Date.now();
  if (tx) { current = a; a.begin('Simulating and proving in this tab'); lane('tab', 'proving', 'busy'); }
  else { a.begin('In this tab'); lane('tab', what, 'busy'); }
  try {
    const out = await body();
    const block = out?.receipt?.blockNumber;
    if (tx && block !== undefined) {
      a.end(`In block ${block}`);
      lane('node', `your transaction is in block ${block}`, 'done');
      a.begin('Settling on JAM');
      watching.push({ a, block: Number(block), stage: 'settle' });
    } else a.end();
    if (!tx) lane('tab', 'idle');
    say(`${what} — done in ${((Date.now() - t0) / 1000).toFixed(1)} s`, 'done');
    return out;
  } catch (e) {
    console.error(e);
    a.end(`Failed: ${e.message ?? e}`, 'bad');
    lane('tab', 'the last action failed', 'bad');
    say(`error: ${what}: ${e.message ?? e}`, 'error');
    throw e;
  } finally {
    current = undefined;
    for (const b of document.querySelectorAll('button')) b.disabled = false;
  }
}

// --- the chain, polled: Aztec's tips and the sidecar's milestones -------------------------------
const watching = [];   // transactions in a block, waiting to settle on JAM and then to be proven
let since = 0, lastProven = -1, relayT0, epochT0;

async function pollNode() {
  const t = await node.getChainTips();
  const proposed = Number(t.proposed.number), cp = Number(t.checkpointed.block.number), pr = Number(t.proven.block.number);
  $('net').dataset.block = String(proposed);
  if (quiet('node')) lane('node', `at block ${proposed}`);
  for (const w of [...watching]) {
    if (w.stage === 'settle' && cp >= w.block) {
      w.a.end(`Settled on JAM: checkpoint ${t.checkpointed.checkpoint.number}, verified and finalized by the validators`);
      w.a.begin('Waiting for the epoch proof (Aztec’s prover, then JAM verifies it)');
      w.stage = 'prove';
    }
    if (w.stage === 'prove' && pr >= w.block) {
      w.a.end('Proven: Aztec’s epoch proof, verified on JAM');
      watching.splice(watching.indexOf(w), 1);
    }
  }
  if (lastProven >= 0 && pr > lastProven) lane('jam', `epoch proof verified: proven up to block ${pr}`, 'done');
  lastProven = pr;
}

// the prover node's epoch job states (Aztec's EpochProvingJobState) that are still working
const STAGE = {
  initialized: 'starting', 'awaiting-checkpoints': 'proving its blocks and checkpoints',
  'awaiting-root': 'proving the final (root) proof', 'awaiting-predecessor': 'waiting for the epoch before',
  'publishing-proof': 'sending the proof to JAM',
};

async function pollJam() {
  const r = await fetch(JAM_URL, { method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'jam_activity', params: [since] }) }).then((x) => x.json());
  const { jamSlot, events, prover } = r.result;
  $('net').textContent = `chain ${$('net').dataset.chain} · Aztec block ${$('net').dataset.block ?? '…'} · JAM slot ${jamSlot}`;
  if (quiet('jam')) lane('jam', `finalized up to slot ${jamSlot}`);
  if (quiet('relay')) lane('relay', 'waiting for the next checkpoint');
  for (const e of events) {
    since = Math.max(since, e.t);
    if ('relaying' in e) { relayT0 = e.t; lane('relay', 'handing a checkpoint to JAM', 'busy', e.t); }
    else if ('submitted' in e) {
      if (e.code === 0) {
        lane('relay', `JAM took the checkpoint in ${dur(e.t - (relayT0 ?? e.t))}`, 'done');
        lane('jam', 'verifying the checkpoint (refine), recording it (accumulate)', 'busy', e.t);
      } else lane('relay', 'JAM refused the work package', 'bad');
    } else if ('checkpoint' in e) lane('jam', `checkpoint ${e.checkpoint} verified, recorded and finalized`, 'done');
    else if ('refused' in e) lane('jam', `refused a checkpoint (reason ${e.reason})`, 'bad');
    else if ('relayingEpochProof' in e) {
      lane('prover', `epoch proved (checkpoints ${e.start}–${e.end})`, 'done');
      lane('relay', 'handing the epoch proof to JAM', 'busy', e.t);
    } else if ('submittedEpochProof' in e) {
      lane('relay', e.code === 0 ? 'JAM took the epoch proof' : 'JAM refused the epoch proof', e.code === 0 ? 'done' : 'bad');
      if (e.code === 0) lane('jam', 'verifying Aztec’s epoch proof (refine)', 'busy', e.t);
    }
  }
  const job = (prover ?? []).filter((j) => STAGE[j.status]).at(-1);
  if (job) {
    if ($('lane-prover').dataset.job !== job.uuid) { epochT0 = Date.now(); $('lane-prover').dataset.job = job.uuid; }
    const slow = Date.now() - epochT0 > 60_000 ? ' — on a laptop; Aztec’s guidance is 32 cores' : '';
    lane('prover', `epoch ${job.epochNumber}: ${STAGE[job.status]}${slow}`, 'busy', epochT0);
  } else if (quiet('prover') || $('lane-prover').dataset.state === 'busy') lane('prover', prover ? 'waiting for the next epoch' : 'not running');
}

async function poll() {
  await pollNode().catch((e) => lane('node', `not reachable (${e.message ?? e})`, 'bad'));
  await pollJam().catch(() => lane('relay', 'sidecar not reachable', 'bad'));
  setTimeout(poll, 2000);
}

// --- the wallet ----------------------------------------------------------------------------------
const fee = () => ({ paymentMethod: new SponsoredFeePaymentMethod(fpcAddress) });
const showAddress = () => { $('address').textContent = me.toString(); $('address').title = me.toString(); };
const showSenders = async () => {
  const list = await wallet.getAddressBook();
  $('senders').replaceChildren(...list.map((s) => {
    const li = document.createElement('li'); li.textContent = short(s.item); li.title = s.item.toString(); return li;
  }));
};

// the token in use is remembered per node, so a reload comes back to it (the account and notes
// persist on their own, in the PXE's store); storage may be unavailable, which only loses that
const TOKEN_KEY = `token@${NODE_URL}`;
const remember = (address) => { try { localStorage.setItem(TOKEN_KEY, address.toString()); } catch {} explorerLink(); };
const forget = () => { try { localStorage.removeItem(TOKEN_KEY); } catch {} };
const recall = () => { try { return localStorage.getItem(TOKEN_KEY); } catch { return null; } };

/** the explorer, told where everything is (never any keys: those are pasted, so no URL holds them) */
function explorerLink() {
  const q = new URLSearchParams({ node: NODE_URL, jam: JAM_URL });
  if (params.get('jip2')) q.set('jip2', params.get('jip2'));
  if (token) q.set('token', token.address.toString());
  $('explorer').href = `${EXPLORER_URL}/?${q}`;
}

const needAccount = () => { if (!me) throw new Error('create an account first'); return me; };
const needToken = () => {
  needAccount();
  if (!token) throw new Error('no token yet: deploy a new token, or paste a token address and press "Use this token"');
  return token;
};

async function attach(address) {
  // the instance was published on deployment; the node serves its preimage to any PXE
  const instance = await node.getContract(address);
  if (!instance) throw new Error(`the node has no contract at ${address}`);
  await wallet.registerContract(instance, TokenContract.artifact);
  token = await TokenContract.at(address, wallet);
  $('token-address').value = address.toString();
  remember(address);
}

async function refresh() {
  if (!token || !me) return;
  // a simulation syncs the PXE to the chain's head, then reads the balance
  const [priv, pub] = await Promise.all([
    token.methods.balance_of_private(me).simulate({ from: me }),
    token.methods.balance_of_public(me).simulate({ from: me }),
  ]);
  $('balance').textContent = String(priv.result);
  $('public-balance').textContent = String(pub.result);
}

async function boot() {
  node = createAztecNodeClient(NODE_URL);
  explorerLink();
  const info = await node.getNodeInfo();
  $('net').dataset.chain = String(info.l1ChainId);
  $('net').textContent = `chain ${info.l1ChainId}`;
  for (const id of ['tab', 'node', 'relay', 'jam', 'prover']) lane(id, '—');
  poll();
  say('opening the wallet (Aztec PXE in this tab)');
  lane('tab', 'opening Aztec’s PXE', 'busy');
  wallet = await EmbeddedWallet.create(NODE_URL, { pxe: { proverEnabled: true } });
  instrument(wallet);
  const fpc = await getContractInstanceFromInstantiationParams(SponsoredFPCContract.artifact, { salt: new Fr(SPONSORED_FPC_SALT) });
  fpcAddress = fpc.address;
  await wallet.registerContract(fpc, SponsoredFPCContract.artifact);
  // a transaction anchors to the PXE's synced block; one anchored to genesis is valid only in
  // block 1, so wait for the chain's first block before acting
  say('waiting for the chain’s first block');
  while ((await node.getBlockNumber()) < 1) await new Promise((r) => setTimeout(r, 2000));
  const accounts = await wallet.getAccounts();
  if (accounts.length) { me = accounts[0].item; showAddress(); await showSenders(); }
  const saved = recall();
  if (saved) {
    // a restarted chain no longer has the contract: forget it rather than fail every action
    try { await attach(AztecAddress.fromStringUnsafe(saved)); } catch (e) { console.error('the remembered token', e); forget(); }
    await refresh().catch((e) => console.error('reading balances after a reload', e));
  }
  lane('tab', 'ready', 'idle');
  say(accounts.length ? 'ready' : 'ready — create an account', 'done');
  document.body.dataset.ready = '1';
}

// --- tabs ----------------------------------------------------------------------------------------
for (const t of document.querySelectorAll('[role=tab]')) {
  t.onclick = () => {
    for (const u of document.querySelectorAll('[role=tab]')) u.setAttribute('aria-selected', String(u === t));
    for (const p of document.querySelectorAll('[data-panel]')) p.hidden = p.dataset.panel !== t.dataset.tab;
  };
}

// --- actions -------------------------------------------------------------------------------------
const amount = (id) => BigInt($(id).value);

$('create').onclick = () => run('Create an account', async () => {
  const manager = await wallet.createSchnorrInitializerlessAccount(Fr.random(), Fr.random(), Fq.random(), 'me');
  me = manager.address; showAddress();
});

$('deploy').onclick = () => run('Deploy a token', async () => {
  const out = await TokenContract.deploy(wallet, needAccount(), 'Rebel Token', 'RBL', 6).send({ from: me, fee: fee(), wait });
  token = out.contract; $('token-address').value = token.address.toString();
  remember(token.address);
  await refresh();
  return out;
}, { tx: true });

$('attach').onclick = () => run('Use a token', async () => {
  const address = $('token-address').value.trim();
  if (!address) throw new Error('paste a token address first');
  await attach(AztecAddress.fromStringUnsafe(address));
  await refresh();
});

$('mint').onclick = () => run(`Mint ${$('mint-amount').value} privately`, async () => {
  const out = await needToken().methods.mint_to_private(me, amount('mint-amount')).send({ from: me, fee: fee(), wait });
  await refresh(); return out;
}, { tx: true });

$('mint-public').onclick = () => run(`Mint ${$('mint-amount').value} publicly`, async () => {
  const out = await needToken().methods.mint_to_public(me, amount('mint-amount')).send({ from: me, fee: fee(), wait });
  await refresh(); return out;
}, { tx: true });

$('send').onclick = () => run(`Send ${$('send-amount').value} privately`, async () => {
  const to = AztecAddress.fromStringUnsafe($('send-to').value.trim());
  const out = await needToken().methods.transfer(to, amount('send-amount')).send({ from: me, fee: fee(), wait });
  await refresh(); return out;
}, { tx: true });

$('shield').onclick = () => run(`Shield ${$('shield-amount').value}`, async () => {
  const out = await needToken().methods.transfer_to_private(me, amount('shield-amount')).send({ from: me, fee: fee(), wait });
  await refresh(); return out;
}, { tx: true });

$('unshield').onclick = () => run(`Unshield ${$('unshield-amount').value}`, async () => {
  const out = await needToken().methods.transfer_to_public(me, me, amount('unshield-amount'), 0).send({ from: me, fee: fee(), wait });
  await refresh(); return out;
}, { tx: true });

$('add-sender').onclick = () => run('Register a sender', async () => {
  await wallet.registerSender(AztecAddress.fromStringUnsafe($('sender-address').value.trim()), 'sender');
  await showSenders();
});

$('show-keys').onclick = () => run('Show viewing keys', async () => {
  $('keys-text').value = await exportViewKeys(wallet, needAccount(), { token: token?.address });
  $('keys-box').hidden = false; $('show-keys').hidden = true;
});
$('hide-keys').onclick = () => { $('keys-text').value = ''; $('keys-box').hidden = true; $('show-keys').hidden = false; };
$('copy-keys').onclick = () => run('Copy viewing keys', () => navigator.clipboard.writeText($('keys-text').value));

$('refresh').onclick = () => run('Sync and read balances', refresh);

boot().catch((e) => { console.error(e); lane('tab', 'failed to open', 'bad'); say(`error: ${e.message ?? e}`, 'error'); });
