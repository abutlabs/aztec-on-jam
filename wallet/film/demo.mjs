// The demo film: Alice and Bob in the wallet, on Aztec's stack settling on JAM, each page with its
// live view of who is doing what. Every stage is its own screencast, sped up by how long it waits,
// so a wait stays on screen with its busy timer running; then the closing card. Into demo.mp4 and
// demo.gif. Needs the stack up on a fresh net (tools/aztec-local/on-jam.sh), the wallet built
// (npm run build) and ffmpeg on PATH.
//   node wallet/film/demo.mjs   (NODE_URL=http://localhost:8080, PORT=19721, IMAGES=docs/images, CHROME)
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer-core';
import { Film, caption } from './film.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CHROME = process.env.CHROME || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const PORT = Number(process.env.PORT ?? 19721);
const NODE_URL = process.env.NODE_URL || 'http://localhost:8080';
const WALLET = `http://localhost:${PORT}/?node=${encodeURIComponent(NODE_URL)}`;
const IMAGES = process.env.IMAGES ? path.resolve(process.env.IMAGES) : path.resolve(HERE, '../../docs/images');

const server = spawn(process.execPath, [path.join(HERE, '..', 'serve.mjs'), String(PORT)], { stdio: 'inherit' });
const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, protocolTimeout: 1_800_000 });
const film = new Film(browser, path.join(IMAGES, 'demo-parts'));
const fail = async (msg) => { console.log(`FAIL ${msg}`); await browser.close(); server.kill(); process.exit(1); };
// the DOM is watched for changes: a page in the background runs no animation frames to poll on
const until = (page, fn, timeout, ...args) => page.waitForFunction(fn, { polling: 'mutation', timeout }, ...args);

/** A user's wallet in a browser context of its own; 1280×800 scales to the film's 1100×688. */
async function open(user) {
  const page = await (await browser.createBrowserContext()).newPage();
  await page.setViewport({ width: 1280, height: 800 });
  page.on('pageerror', (e) => console.log(`  [${user}] pageerror: ${String(e).slice(0, 200)}`));
  await page.goto(WALLET);
  await until(page, () => document.body.dataset.ready === '1' || document.getElementById('status').className === 'error', 300_000);
  console.log(`  ${user}: wallet ready`);
  return page;
}

/** Opens the button's tab, fills its fields, clicks, and waits for the action to finish. */
async function act(page, button, fill = {}) {
  const tab = await page.$eval(button, (b) => b.closest('[data-panel]')?.dataset.panel ?? null);
  if (tab) { await page.click(`[data-tab=${tab}]`); await page.waitForSelector(button, { visible: true }); }
  for (const [sel, v] of Object.entries(fill)) await page.$eval(sel, (e, v) => { e.value = v; }, String(v));
  await page.click(button);
  await until(page, () => !document.getElementById('status').textContent.startsWith('working'), 600_000);
  const s = await page.$eval('#status', (e) => e.textContent);
  console.log(`  ${s}`);
  if (s.startsWith('error')) await fail(s);
}

/** One stage: the page in front, captioned, filmed while `body` runs, played `speed` times as
 *  fast; then its result held for a beat at real speed, however fast the wait before it played. */
async function stage(name, page, text, speed, body) {
  await page.bringToFront();
  await caption(page, text);
  await film.cast(name, page);
  await body();
  await film.stop(name);
  film.centre(name, speed);
  await film.cast(`${name}-result`, page);
  await new Promise((r) => setTimeout(r, 2500));
  await film.stop(`${name}-result`);
  film.centre(`${name}-result`, 1);
}

/** Scrolls so the JAM validators' and prover's lanes and the newest activity share the frame. */
const toJam = (page) => page.evaluate(() => window.scrollTo(0, document.getElementById('lane-jam').getBoundingClientRect().top + scrollY - 104));

const settled = (label) => [...document.querySelectorAll('#log > li')].some((li) =>
  li.querySelector('b').textContent.startsWith(label) && /Settled on JAM/.test(li.textContent));
const proven = (label) => [...document.querySelectorAll('#log > li')].some((li) =>
  li.querySelector('b').textContent.startsWith(label) && /Proven:/.test(li.textContent));

const alice = await open('alice');
const bob = await open('bob');

await stage('alice-token', alice, 'Alice makes a token and mints 1000 to herself, privately', 3, async () => {
  await act(alice, '#create');
  await act(alice, '#deploy');
  await act(alice, '#mint', { '#mint-amount': 1000 });
});
const token = await alice.$eval('#token-address', (e) => e.value);
const aliceAddr = await alice.$eval('#address', (e) => e.textContent);

await stage('bob-open', bob, 'Bob opens his wallet and adds Alice as a sender he expects', 2, async () => {
  await act(bob, '#create');
  await act(bob, '#add-sender', { '#sender-address': aliceAddr });
  await act(bob, '#attach', { '#token-address': token });
});
const bobAddr = await bob.$eval('#address', (e) => e.textContent);

await stage('alice-send', alice, 'Alice sends Bob 40, privately: the proof is made in her browser', 3, async () => {
  await act(alice, '#send', { '#send-to': bobAddr, '#send-amount': 40 });
});
await toJam(alice);
await stage('alice-settle', alice, 'JAM’s validators verify the checkpoint and finalize it', 8, async () => {
  await until(alice, settled, 300_000, 'Send 40');
});
await stage('bob-receive', bob, 'Bob finds the 40 with his own key; the chain shows no amount or names', 2, async () => {
  await act(bob, '#refresh');
  if ((await bob.$eval('#balance', (e) => e.textContent)) !== '40') await fail('Bob does not see 40');
});
// Bob's part is done: his tab (a PXE and a prover) closes, to spare the memory the epoch proof needs
await bob.close();
await toJam(alice);
await stage('alice-proven', alice, 'Aztec’s prover proves the whole epoch; JAM verifies that proof too', 80, async () => {
  await until(alice, proven, 3_600_000, 'Send 40');
});

await film.titles('end');
film.render(path.join(IMAGES, 'demo.mp4'), path.join(IMAGES, 'demo.gif'));
console.log(`PASS: film ${path.relative(process.cwd(), IMAGES)}/demo.mp4 and demo.gif`);
await browser.close(); server.kill();
process.exit(0);
