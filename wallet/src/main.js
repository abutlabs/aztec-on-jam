// Our wallet, on Aztec's released libraries (#65). Everything cryptographic is Aztec's:
// EmbeddedWallet (browser build: the PXE in this tab, SQLite on OPFS), Aztec's prover (bb.js,
// threads — the page must be cross-origin isolated), Aztec's Token contract, and Aztec's note
// tagging for discovery. Fees go through the network's SponsoredFPC, funded at genesis, so a
// fresh account can act with no funds. The node URL comes from ?node=, default :8080.
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { getContractInstanceFromInstantiationParams } from '@aztec-labs/aztec.js/contracts';
import { AztecAddress } from '@aztec-labs/aztec.js/addresses';
import { SponsoredFeePaymentMethod } from '@aztec-labs/aztec.js/fee';
import { Fr, Fq } from '@aztec-labs/aztec.js/fields';
import { SPONSORED_FPC_SALT } from '@aztec-labs/constants';
import { SponsoredFPCContract } from '@aztec-labs/noir-contracts.js/SponsoredFPC';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';

const $ = (id) => document.getElementById(id);
const say = (text, cls = '') => { $('status').textContent = text; $('status').className = cls; };
const log = (text) => {
  const li = document.createElement('li');
  const t = document.createElement('span'); t.className = 't'; t.textContent = new Date().toLocaleTimeString();
  li.append(t, text); $('log').prepend(li);
};
const NODE_URL = new URLSearchParams(location.search).get('node') ?? 'http://localhost:8080';
const wait = { timeout: 600, interval: 2 };

let node, wallet, me, token, fpcAddress;

/** Every action runs through here: one at a time, status + log + timing, errors surfaced. */
async function run(what, body) {
  for (const b of document.querySelectorAll('button')) b.disabled = true;
  say(`working: ${what}`, ''); log(`${what}…`);
  const t0 = Date.now();
  try {
    const out = await body();
    const s = `${what} — done in ${((Date.now() - t0) / 1000).toFixed(1)} s`;
    say(s, 'done'); log(s);
    return out;
  } catch (e) {
    console.error(e);
    say(`error: ${what}: ${e.message ?? e}`, 'error'); log(`${what} FAILED: ${e.message ?? e}`);
    throw e;
  } finally {
    for (const b of document.querySelectorAll('button')) b.disabled = false;
  }
}

const fee = () => ({ paymentMethod: new SponsoredFeePaymentMethod(fpcAddress) });
const showAddress = () => { $('address').textContent = me.toString(); };
const showSenders = async () => {
  const list = await wallet.getAddressBook();
  $('senders').replaceChildren(...list.map((s) => {
    const li = document.createElement('li'); li.textContent = s.item.toString(); return li;
  }));
};

async function attach(address) {
  // the instance was published on deployment; the node serves its preimage to any PXE
  const instance = await node.getContract(address);
  if (!instance) throw new Error(`the node has no contract at ${address}`);
  await wallet.registerContract(instance, TokenContract.artifact);
  token = await TokenContract.at(address, wallet);
  $('token-address').value = address.toString();
}

async function refresh() {
  if (!token || !me) return;
  // a simulation syncs the PXE to the chain's head, then reads the private balance
  const { result } = await token.methods.balance_of_private(me).simulate({ from: me });
  $('balance').textContent = String(result);
}

async function boot() {
  node = createAztecNodeClient(NODE_URL);
  const info = await node.getNodeInfo();
  $('net').textContent = `${NODE_URL} · chain ${info.l1ChainId} · rollup v${info.rollupVersion}`;
  say('opening the wallet (Aztec PXE in this tab)');
  wallet = await EmbeddedWallet.create(NODE_URL, { pxe: { proverEnabled: true } });
  const fpc = await getContractInstanceFromInstantiationParams(SponsoredFPCContract.artifact, { salt: new Fr(SPONSORED_FPC_SALT) });
  fpcAddress = fpc.address;
  await wallet.registerContract(fpc, SponsoredFPCContract.artifact);
  const accounts = await wallet.getAccounts();
  if (accounts.length) { me = accounts[0].item; showAddress(); await showSenders(); }
  const block = await node.getBlockNumber();
  log(`ready: node at block ${block}; sponsor ${fpcAddress.toString().slice(0, 10)}…`);
  say(accounts.length ? 'ready' : 'ready — create an account', 'done');
  document.body.dataset.ready = '1';
}

$('create').onclick = () => run('create account (initializerless, in this tab)', async () => {
  const manager = await wallet.createSchnorrInitializerlessAccount(Fr.random(), Fr.random(), Fq.random(), 'me');
  me = manager.address; showAddress();
});

$('deploy').onclick = () => run('deploy Aztec Token (proved in this tab, fee sponsored)', async () => {
  const { contract } = await TokenContract.deploy(wallet, me, 'Rebel Token', 'RBL', 6)
    .send({ from: me, fee: fee(), wait });
  token = contract; $('token-address').value = contract.address.toString();
  await refresh();
});

$('attach').onclick = () => run('attach token', async () => {
  await attach(AztecAddress.fromStringUnsafe($('token-address').value.trim()));
  await refresh();
});

$('mint').onclick = () => run('mint privately (proved in this tab, fee sponsored)', async () => {
  await token.methods.mint_to_private(me, BigInt($('mint-amount').value)).send({ from: me, fee: fee(), wait });
  await refresh();
});

$('send').onclick = () => run('send privately (proved in this tab, fee sponsored)', async () => {
  const to = AztecAddress.fromStringUnsafe($('send-to').value.trim());
  await token.methods.transfer(to, BigInt($('send-amount').value)).send({ from: me, fee: fee(), wait });
  await refresh();
});

$('add-sender').onclick = () => run('register sender (their notes reach me by Aztec tagging)', async () => {
  await wallet.registerSender(AztecAddress.fromStringUnsafe($('sender-address').value.trim()), 'sender');
  await showSenders();
});

$('refresh').onclick = () => run('sync and read the private balance', refresh);

boot().catch((e) => { console.error(e); say(`error: ${e.message ?? e}`, 'error'); });
