// The wallet's check, in real browsers. Alice and Bob each open the wallet in a browser context
// of their own (separate storage, as two users). Alice creates an account, deploys Aztec's Token,
// mints 1000 privately, unshields 100 and shields it back; Bob creates an account and registers
// Alice as a sender; Alice sends Bob 40 privately; Bob's wallet finds the note by Aztec's
// tagging; Alice's activity shows the send settled on JAM. Then the explorer, in a third context
// with no wallet: it shows the send's transaction with no sender, recipient or amount, and once
// Bob's viewing keys are pasted, his balance, the payment from Alice, and which transaction it was.
// Every proof is a real Aztec client proof made in the tab (proverEnabled), so the page must be
// cross-origin isolated. Needs: the devnet up (tools/aztec-local/on-jam.sh), both pages built.
//   node e2e.mjs      (NODE_URL=http://localhost:8080, CHROME=system Chrome, PORT=19720,
//                      EXPLORER_PORT=PORT+10, JIP2=ws://localhost:42950)
import { execFileSync, spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import puppeteer from 'puppeteer-core';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const PORT = Number(process.env.PORT ?? 19720);
const NODE_URL = process.env.NODE_URL || 'http://localhost:8080';
const EXPLORER_PORT = Number(process.env.EXPLORER_PORT ?? PORT + 10);
const JIP2 = process.env.JIP2 || 'ws://localhost:42950';
const WALLET = `http://localhost:${PORT}/?${new URLSearchParams({ node: NODE_URL, explorer: `http://localhost:${EXPLORER_PORT}`, jip2: JIP2 })}`;

const servers = [spawn(process.execPath, [path.join(HERE, 'serve.mjs'), String(PORT)], { stdio: 'inherit' }),
  spawn(process.execPath, [path.join(HERE, 'serve.mjs'), String(EXPLORER_PORT), 'explorer.html'], { stdio: 'inherit' })];
const server = { kill: () => servers.forEach((s) => s.kill()) };
// waits watch the DOM for changes: a tab in the background runs no animation frames to poll on
const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, protocolTimeout: 900000, defaultViewport: { width: 1000, height: 1100 } });
let pass = 0;
const fail = async (msg) => { console.log(`  FAIL ${msg}`); await browser.close(); server.kill(); process.exit(1); };
const ok = (name, cond, detail = '') => { if (!cond) return fail(`${name}${detail ? ': ' + detail : ''}`); pass++; console.log(`  ok   ${name}${detail ? ': ' + detail : ''}`); };

async function open(user) {
  const page = await (await browser.createBrowserContext()).newPage();
  page.on('pageerror', (e) => console.log(`  [${user}] pageerror: ${String(e).slice(0, 200)}`));
  page.on('console', (m) => { if (m.type() === 'error') console.log(`  [${user}] console: ${m.text().slice(0, 200)}`); });
  await page.goto(WALLET);
  await page.waitForFunction(() => document.body.dataset.ready === '1' || document.getElementById('status').className === 'error', { polling: 'mutation', timeout: 300000 });
  if (await page.$eval('#status', (e) => e.className) === 'error') await fail(`${user}: ${await page.$eval('#status', (e) => e.textContent)}`);
  console.log(`  ${user}: wallet ready (PXE in the tab)`);
  return page;
}

/** Clicks a button and waits for the action to finish; returns the seconds the page reports. */
async function act(page, user, button, fill = {}) {
  // the acting user's page in front (a background tab runs no animation frames, which
  // puppeteer's visibility wait polls on), and a button on a tab's panel: open that tab first
  await page.bringToFront();
  const tab = await page.$eval(button, (b) => b.closest('[data-panel]')?.dataset.panel ?? null);
  if (tab) { await page.click(`[data-tab=${tab}]`); await page.waitForSelector(button, { visible: true }); }
  for (const [sel, v] of Object.entries(fill)) await page.$eval(sel, (e, v) => { e.value = v; }, String(v));
  await page.click(button);
  await page.waitForFunction(() => /^(working|error)/.test(document.getElementById('status').textContent), { polling: 'mutation', timeout: 10000 });
  await page.waitForFunction(() => !document.getElementById('status').textContent.startsWith('working'), { polling: 'mutation', timeout: 600000 });
  const s = await page.$eval('#status', (e) => e.textContent);
  console.log(`  [${user}] ${s}`);
  if (s.startsWith('error')) await fail(s);
  return Number(s.match(/in ([0-9.]+) s/)?.[1] ?? NaN);
}

const text = (page, sel) => page.$eval(sel, (e) => e.textContent);
const value = (page, sel) => page.$eval(sel, (e) => e.value);

const alice = await open('alice');
const bob = await open('bob');

await act(alice, 'alice', '#create');
const aliceAddr = await text(alice, '#address');
await act(bob, 'bob', '#create');
const bobAddr = await text(bob, '#address');
ok('two accounts, created in their tabs, never deployed', /^0x[0-9a-f]{64}$/.test(aliceAddr) && /^0x[0-9a-f]{64}$/.test(bobAddr));

// minting before any token is chosen says what to do, not a TypeError
await bob.bringToFront();
await bob.click('[data-tab=token]');
await bob.click('#mint');
await bob.waitForFunction(() => document.getElementById('status').className === 'error', { polling: 'mutation', timeout: 10000 });
const noToken = await text(bob, '#status');
ok('mint with no token chosen explains itself', /no token yet/.test(noToken), noToken);

const tDeploy = await act(alice, 'alice', '#deploy');
const tokenAddr = await value(alice, '#token-address');
ok('alice deployed Aztec\'s Token, fee sponsored', /^0x[0-9a-f]{64}$/.test(tokenAddr), `${tokenAddr.slice(0, 10)}… in ${tDeploy}s`);

const tMint = await act(alice, 'alice', '#mint', { '#mint-amount': 1000 });
ok('alice minted 1000 privately', (await text(alice, '#balance')) === '1000', `${tMint}s`);

const tUnshield = await act(alice, 'alice', '#unshield', { '#unshield-amount': 100 });
ok('alice unshielded 100: private 900, public 100', (await text(alice, '#balance')) === '900' && (await text(alice, '#public-balance')) === '100', `${tUnshield}s`);
const tShield = await act(alice, 'alice', '#shield', { '#shield-amount': 100 });
ok('alice shielded 100 back: private 1000, public 0', (await text(alice, '#balance')) === '1000' && (await text(alice, '#public-balance')) === '0', `${tShield}s`);

await act(bob, 'bob', '#add-sender', { '#sender-address': aliceAddr });
await act(bob, 'bob', '#attach', { '#token-address': tokenAddr });
ok('bob attached the token and registered alice as a sender', (await text(bob, '#balance')) === '0');

const tSend = await act(alice, 'alice', '#send', { '#send-to': bobAddr, '#send-amount': 40 });
ok('alice sent 40 privately', (await text(alice, '#balance')) === '960', `${tSend}s, alice 960`);

// a proposed block can be pruned (its checkpoint not proposed in time) and its transaction
// re-included a block later: sync again until the note is there
let syncs = 0;
do { if (syncs) await new Promise((r) => setTimeout(r, 10000)); await act(bob, 'bob', '#refresh'); syncs++; }
while ((await text(bob, '#balance')) !== '40' && syncs < 9);
ok('bob found the 40 by Aztec tagging, no key shared', (await text(bob, '#balance')) === '40',
  syncs > 1 ? `after ${syncs} syncs: the first block was pruned and the send re-included` : '');

// the explorer, from Bob's wallet's link, in a context of its own: no wallet, no keys
await act(bob, 'bob', '#show-keys');
const bobKeys = await value(bob, '#keys-text');
const bundle = JSON.parse(bobKeys);
ok('bob\'s wallet shows his viewing keys, with alice as a sender and no signing key',
  bundle.kind === 'aztec-view-keys/1' && bundle.address === bobAddr && bundle.senders.includes(aliceAddr)
  && !Object.keys(bundle).some((k) => /signing(Secret)?Key$|^secret/i.test(k) && !/PublicKey$/.test(k)), Object.keys(bundle).join(' '));
const explorerUrl = await bob.$eval('#explorer', (a) => a.href);
const ledger = await (await browser.createBrowserContext()).newPage();
ledger.on('pageerror', (e) => console.log(`  [ledger] pageerror: ${String(e).slice(0, 200)}`));
await ledger.goto(explorerUrl);
await ledger.waitForFunction(() => document.body.dataset.ready === '1' || document.getElementById('status').className === 'error', { polling: 'mutation', timeout: 120000 });
await ledger.waitForFunction(() => document.querySelectorAll('#blocks .block:not(.empty) .tx').length > 0, { polling: 'mutation', timeout: 30000 });
ok('the explorer opens from the wallet\'s link and lists transactions, with no keys', !(await ledger.$('.yours:not(:empty)')),
  `${await ledger.$$eval('#blocks .block:not(.empty)', (b) => b.length)} blocks with transactions`);
ok('total supply is public', /Total supply: 1000 RBL/.test(await text(ledger, '#supply')), await text(ledger, '#supply'));
await act(ledger, 'ledger', '#unlock', { '#keys': bobKeys });
await ledger.waitForFunction(() => document.getElementById('priv').textContent === '40'
  && /received 40 from/.test(document.getElementById('history').textContent), { polling: 'mutation', timeout: 120000 });
ok('bob\'s keys in the explorer: his private balance and the payment from alice', true,
  `${await text(ledger, '#priv')} private · ${await text(ledger, '#history li b')}`);
ok('the keys leave the text box once read', (await value(ledger, '#keys')) === '');
await ledger.waitForFunction(() => /^yours: received 40/.test(document.querySelector('.yours:not(:empty)')?.textContent ?? ''), { polling: 'mutation', timeout: 30000 });
ok('the public ledger marks that same transaction as his', true, await ledger.$eval('.yours:not(:empty)', (e) => e.closest('.tx').querySelector('.mono').textContent));

// the live view: the send's own timeline reaches JAM
await alice.waitForFunction(() => [...document.querySelectorAll('#log > li')].some((li) =>
  /^Send 40/.test(li.querySelector('b').textContent) && /Settled on JAM: checkpoint \d+/.test(li.textContent)), { polling: 'mutation', timeout: 300000 });
ok('alice\'s activity shows the send settled on JAM', true,
  await alice.$eval('#log', (l) => [...l.querySelectorAll('.steps li')].slice(0, 5).map((s) => s.textContent).join(' | ')));

// a reload keeps the account and the balance (the wallet's store is SQLite on OPFS) and comes back
// to the token in use, with no attach by hand
await alice.reload();
await alice.waitForFunction(() => document.body.dataset.ready === '1', { polling: 'mutation', timeout: 120000 });
ok('alice reloads: account, token and notes persist', (await text(alice, '#address')) === aliceAddr
  && (await value(alice, '#token-address')) === tokenAddr && (await text(alice, '#balance')) === '960');

console.log(`PASS: ${pass} assertions; deploy ${tDeploy}s, mint ${tMint}s, unshield ${tUnshield}s, shield ${tShield}s, send ${tSend}s (proofs made in the tab)`);
await browser.close(); server.kill();
process.exit(0);
