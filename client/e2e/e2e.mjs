// The wallet, driven like a person would: wallet A in one browser context, wallet B in
// another (separate storage, so separate secrets). Each page holds its own keys, proves its
// own transfers in the browser and finds its notes by scanning the chain; the only thing they
// share is the chain. A pays B at the address B's page shows. Asserts every balance the pages
// show, relative to where they started, and saves a screenshot per move to docs/images/.
// Exit 0 only if every assertion holds.
//   node client/e2e/e2e.mjs            (WALLET=http://localhost:19710, CHROME=system Chrome)
import puppeteer from "puppeteer-core";
import { mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const WALLET = process.env.WALLET || "http://localhost:19710";
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const IMAGES = process.env.IMAGES ? path.resolve(process.env.IMAGES) : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../docs/images");   // IMAGES= elsewhere: a run that should not replace the README's
// The README's recording is explorer.mjs's (RECORD=1 there): Alice and Bob, then the explorer.
mkdirSync(IMAGES, { recursive: true });

const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, defaultViewport: { width: 900, height: 1300 } });
let pass = 0;
const results = [];
const fail = async (msg) => { console.log("  FAIL " + msg); await browser.close(); process.exit(1); };

async function open(user) {
  const ctx = await browser.createBrowserContext();               // a second browser, in effect
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.log(`  [${user}] pageerror: ${String(e).slice(0, 200)}`));
  await page.goto(`${WALLET}/?user=${user}`);
  // ready: keys made, address published on chain, balances shown
  await page.waitForFunction(() => document.body.dataset.ready === "1" || document.getElementById("status").textContent.startsWith("error"), { timeout: 240000 });
  const status = await page.$eval("#status", (e) => e.textContent);
  if (status.startsWith("error")) await fail(`${user}: ${status}`);
  return page;
}
const num = (page, id) => page.$eval("#" + id, (e) => Number(e.textContent));
async function balances(page) { return { public: await num(page, "public"), private: await num(page, "private") }; }
async function settle(page, want) {                                 // the page polls every 3 s
  const t0 = Date.now();
  while (Date.now() - t0 < 30000) {
    const b = await balances(page);
    if (b.public === want.public && b.private === want.private) return b;
    await new Promise((r) => setTimeout(r, 1000));
  }
  return balances(page);
}
function check(name, got, want) {
  const ok = got.public === want.public && got.private === want.private;
  console.log(`  ${ok ? "ok  " : "FAIL"} ${name}: public ${got.public} (want ${want.public}), private ${got.private} (want ${want.private})`);
  results.push({ name, ok });
  if (ok) pass++; else return fail(name);
}
async function act(page, label, fill) {
  if (fill) await fill(page);
  await page.click(label.button);
  await page.waitForFunction(() => document.getElementById("status").textContent.startsWith("working"), { timeout: 10000 });
  await page.waitForFunction(() => /^(done|error)/.test(document.getElementById("status").textContent), { timeout: 300000 });
  const status = await page.$eval("#status", (e) => e.textContent);
  console.log(`  ${status}`);
  if (status.startsWith("error")) return fail(`${label.name}: ${status}`);
}
async function shot(page, file) {
  await page.screenshot({ path: path.join(IMAGES, file), fullPage: true });
  console.log(`  screenshot ${path.relative(process.cwd(), path.join(IMAGES, file))}`);
}
const setValue = (sel, v) => async (page) => { await page.$eval(sel, (e, v) => { e.value = v; }, String(v)); };

const a = await open("A");
const b = await open("B");
const bAddress = await b.$eval("#address", (e) => e.textContent.trim());
const aAddress = await a.$eval("#address", (e) => e.textContent.trim());
console.log(`A's address ${aAddress.slice(0, 16)}…, B's address ${bAddress.slice(0, 16)}… (each made by its own browser)`);
if (aAddress === bAddress) await fail("two browsers made the same wallet");
const a0 = await balances(a), b0 = await balances(b);
console.log(`start: A ${JSON.stringify(a0)}, B ${JSON.stringify(b0)}`);
await shot(a, "01-a-start.png");

console.log("== A: faucet, mint 100 publicly");
await act(a, { name: "mint", button: "#btn-mint" });
await check("A after mint", await settle(a, { public: a0.public + 100, private: a0.private }), { public: a0.public + 100, private: a0.private });
await shot(a, "02-a-minted.png");

console.log("== A: shield 100 (a deposit by deferred transfer, then a proof that claims it)");
await act(a, { name: "shield", button: "#btn-shield" }, setValue("#amt-shield", 100));
await check("A after shield", await settle(a, { public: a0.public, private: a0.private + 100 }), { public: a0.public, private: a0.private + 100 });
await shot(a, "03-a-shielded.png");

console.log("== A: send 40 to B privately");
await act(a, { name: "send", button: "#btn-send" }, async (p) => { await setValue("#amt-send", 40)(p); await setValue("#to-send", bAddress)(p); });
await check("A after send", await settle(a, { public: a0.public, private: a0.private + 60 }), { public: a0.public, private: a0.private + 60 });
await shot(a, "04-a-sent.png");

console.log("== B, in the other browser: its page finds the 40 by scanning the chain");
await check("B after receiving", await settle(b, { public: b0.public, private: b0.private + 40 }), { public: b0.public, private: b0.private + 40 });
await shot(b, "05-b-received.png");

console.log("== B: unshield 40 to B's public account");
await act(b, { name: "unshield", button: "#btn-unshield" }, setValue("#amt-unshield", 40));
await check("B after unshield", await settle(b, { public: b0.public + 40, private: b0.private }), { public: b0.public + 40, private: b0.private });
await shot(b, "06-b-unshielded.png");

const steps = await b.$$eval("#steps li", (els) => els.map((e) => e.textContent));
if (!steps.some((t) => t.includes("made in this browser"))) await fail("B's unshield was not proved in the browser");
console.log(`PASS: ${pass} assertions, 6 screenshots; every proof made in its own browser`);
await browser.close();
