// Public against private, end to end, on a seeded net (nets/demo.sh up seeds the dev accounts).
// Alice and Bob each open their wallet in a browser context of their own. Alice pays Bob 25 in
// the open, shields 100, and sends Bob 40 privately; Bob unshields 15. Then the explorer is read
// three ways, and what each shows is asserted:
//   no key            the public payment, the shield and the payout are there with names and
//                     amounts; Alice's private send is not in Alice's activity, and no private
//                     transfer carries an amount
//   Alice's keys      the send appears in Alice's activity: it spends her 100 and leaves her 60
//   Bob's viewing key Bob received 40 and 25 (65); it cannot see that he spent the 40
// Screenshots go to IMAGES (docs/images by default). Exit 0 only if every assertion holds.
// RECORD=1 also films it for the README (film.mjs): the card of what is running, Alice's and
// Bob's wallets side by side with captions, the explorer, the closing card; into IMAGES/demo.mp4
// and demo.gif. Needs ffmpeg on PATH. Record on a fresh net (nets/demo.sh down; up), so the
// explorer shows only this story. The opening crawl is a film of its own (film.mjs).
//   node client/e2e/explorer.mjs        (WALLET=http://localhost:19710, CHROME=system Chrome)
import puppeteer from "puppeteer-core";
import { mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { Film, CAPTION, caption, uncaption } from "./film.mjs";

const WALLET = process.env.WALLET || "http://localhost:19710";
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const IMAGES = process.env.IMAGES ? path.resolve(process.env.IMAGES) : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../docs/images");
const RECORD = process.env.RECORD === "1";
mkdirSync(IMAGES, { recursive: true });
// 1250 tall: a wallet's balances, its moves and the status of the one under way, all in view
// (and the film's caption above them)
const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, defaultViewport: { width: 1000, height: 1250 + (RECORD ? CAPTION : 0) } });
let pass = 0;
const fail = async (msg) => { console.log("  FAIL " + msg); await browser.close(); process.exit(1); };
const ok = async (name, cond, detail = "") => { if (!cond) await fail(`${name}${detail ? ": " + detail : ""}`); pass++; console.log(`  ok   ${name}${detail ? ": " + detail : ""}`); };
const privateBalance = (summary) => { const m = summary.match(/Private balance\s+(\d+)/i); return m ? Number(m[1]) : null; };
const shot = async (page, file) => { if (RECORD) await uncaption(page); await page.screenshot({ path: path.join(IMAGES, file), fullPage: true }); console.log(`  screenshot ${file}`); };
const set = (page, sel, v) => page.$eval(sel, (e, v) => { e.value = v; }, String(v));
const num = (page, id) => page.$eval("#" + id, (e) => Number(e.textContent));
const balances = async (page) => ({ public: await num(page, "public"), private: await num(page, "private") });

// ---- the film (RECORD=1 only) ------------------------------------------------------------------
const film = RECORD ? new Film(browser, path.join(IMAGES, "film-parts")) : null;
const say = async (page, text) => { if (film) await caption(page, text); };
const unsay = async (page) => { if (film) await uncaption(page); };
const hold = (s) => film ? new Promise((r) => setTimeout(r, s * 1000)) : Promise.resolve();

async function wallet(user) {
  const page = await (await browser.createBrowserContext()).newPage();
  page.on("pageerror", (e) => console.log(`  [${user}] pageerror: ${String(e).slice(0, 200)}`));
  await page.goto(`${WALLET}/?user=${user}`);
  await page.waitForFunction(() => document.body.dataset.ready === "1" || document.getElementById("status").textContent.startsWith("error"), { timeout: 240000 });
  if ((await page.$eval("#status", (e) => e.textContent)).startsWith("error")) await fail(`${user}: ${await page.$eval("#status", (e) => e.textContent)}`);
  return page;
}
async function act(page, button, fill = {}) {
  for (const [sel, v] of Object.entries(fill)) await set(page, sel, v);
  await page.click(button);
  await page.waitForFunction(() => document.getElementById("status").textContent.startsWith("working"), { timeout: 10000 });
  await page.waitForFunction(() => /^(done|error)/.test(document.getElementById("status").textContent), { timeout: 400000 });
  const s = await page.$eval("#status", (e) => e.textContent);
  console.log(`  ${s}`);
  if (s.startsWith("error")) await fail(s);
}
async function settle(page, want) {
  for (let t0 = Date.now(); Date.now() - t0 < 30000; await new Promise((r) => setTimeout(r, 1000))) {
    const b = await balances(page);
    if (b.public === want.public && b.private === want.private) return b;
  }
  return balances(page);
}
const same = (a, b) => a.public === b.public && a.private === b.private;

if (film) { console.log("== the film's opening: the card"); await film.titles("card"); }
const alice = await wallet("alice"), bob = await wallet("bob");
const a0 = await balances(alice), b0 = await balances(bob);
console.log(`start: Alice ${JSON.stringify(a0)}, Bob ${JSON.stringify(b0)}`);
if (a0.public < 125) await fail("Alice holds under 125 publicly: run nets/demo.sh seed");
if (film) { await film.cast("alice", alice); await film.cast("bob", bob); }
await say(alice, "Alice's rebel wallet: a JAM dev account on PolkaJam's six-validator testnet");
await say(bob, "Bob's wallet, in a browser of its own");
await hold(3);

console.log("== Alice pays Bob 25 publicly");
await say(alice, "Pay Bob 25 in public · the Empire sees Alice, Bob and 25");
await say(bob, "Bob · +25, in plain sight");
await act(alice, "#btn-pay", { "#to-pay": "bob", "#amt-pay": 25 });
let w = { public: a0.public - 25, private: a0.private }; await ok("Alice after paying", same(await settle(alice, w), w), JSON.stringify(w));
w = { public: b0.public + 25, private: b0.private }; await ok("Bob after being paid", same(await settle(bob, w), w), JSON.stringify(w));
console.log("== Alice shields 100, then sends Bob 40 privately");
await say(alice, "Shield 100 · a Noir circuit, proved in this browser by Barretenberg (UltraHonk), verified in JAM's refine");
await say(bob, "Bob · waiting");
await act(alice, "#btn-shield", { "#amt-shield": 100 });
await say(alice, "Send Bob 40 privately · a new proof · the Empire sees two nullifiers and two sealed notes");
await act(alice, "#btn-send", { "#to-send": "bob", "#amt-send": 40 });
w = { public: a0.public - 125, private: a0.private + 60 }; await ok("Alice after shield and send", same(await settle(alice, w), w), JSON.stringify(w));
await say(alice, "Alice keeps 60, in a note only her key opens");
await say(bob, "Bob's wallet tries its key on every sealed note, and finds 40");
w = { public: b0.public + 25, private: b0.private + 40 }; await ok("Bob after the private send", same(await settle(bob, w), w), JSON.stringify(w));
await hold(3);
console.log("== Bob unshields 15");
await say(bob, "Bob unshields 15 · his own proof · accumulate pays his public account by deferred transfer");
await act(bob, "#btn-unshield", { "#amt-unshield": 15 });
w = { public: b0.public + 40, private: b0.private + 25 }; await ok("Bob after unshielding", same(await settle(bob, w), w), JSON.stringify(w));
if (film) { await hold(3); await film.stop("alice", "bob"); film.side("alice", "bob", 4); }   // the chain's waits, at 4x
await unsay(alice); await unsay(bob);
await shot(alice, "11-wallet-alice.png");
const bobVk = await bob.$eval("#viewing-key", (e) => e.textContent.trim());

// ---- the explorer ------------------------------------------------------------------------------
const ex = await (await browser.createBrowserContext()).newPage();
ex.on("pageerror", (e) => console.log(`  [explorer] pageerror: ${String(e).slice(0, 200)}`));
const feed = () => ex.$$eval("#feed li", (els) => els.map((e) => e.innerText));
const text = (id) => ex.$eval("#" + id, (e) => e.innerText);
async function open(url) {
  await ex.goto(url);
  await ex.waitForFunction(() => document.body.dataset.ready === "1", { timeout: 60000 });
  await new Promise((r) => setTimeout(r, 1500));     // the balance and the key's scan
}
/** In the recording: the caption, then the page scrolled down to the activity, slowly. */
async function show(text, s = 6) {
  if (!film) return;
  await say(ex, text);
  await ex.evaluate(() => window.scrollTo(0, 0));
  await hold(1.5);
  await ex.evaluate(() => window.scrollTo({ top: document.getElementById("feed").getBoundingClientRect().top + scrollY - 420, behavior: "smooth" }));
  await hold(s);
}
if (film) await film.cast("explorer", ex);

console.log("== the explorer, no key: Alice's address");
await open(`${WALLET}/explorer.html?address=alice`);
let items = await feed();
await ok("Alice's public payment to Bob is there, named, with its amount", items.some((t) => /Alice paid Bob 25\./.test(t)));
await ok("Alice's shield of 100 is there, with its amount", items.some((t) => /Alice shielded 100\./.test(t)));
await ok("the shield's claim is a private transfer tied to Alice by the shield", items.some((t) => /Private transfer/.test(t) && /the note commitment Alice shielded \(100\)/.test(t)));
await ok("Alice's private send is not in her activity without a key: each private transfer there is a claim of her shield",
  items.filter((t) => /Private transfer/.test(t)).every((t) => /the note commitment Alice shielded/.test(t)));
await ok("no private transfer shows a value", !items.some((t) => /opens to/.test(t)));
await ok("Alice's private balance is hidden", privateBalance(await text("summary")) === null && /hidden/i.test(await text("summary")));
await show("The Imperial Ledger, no key · Alice's public payment and shield are here · her private send to Bob is not");
await shot(ex, "12-explorer-public.png");

console.log("== the explorer, with Alice's keys");
await say(ex, "With Alice's keys · her private send appears, spending her note and keeping 60");
await ex.select("#key", "dev:0");
await ex.waitForFunction(() => /Alice's full keys/.test(document.getElementById("key-note").textContent), { timeout: 30000 });
await new Promise((r) => setTimeout(r, 1500));
items = await feed();
const send = items.find((t) => /spends Alice's note/.test(t) && !/the note commitment Alice shielded/.test(t));
await ok("the private send now shows in Alice's activity, spending her note", Boolean(send));
await ok("it shows Alice's change, 60", /opens to Alice: 60/.test(send || ""));
const aSeen = privateBalance(await text("summary"));
await ok("with her keys, Alice's private balance is what her wallet shows", aSeen === a0.private + 60, `${aSeen}`);
await show("With Alice's keys · her private send appears, spending her note and keeping 60");
await shot(ex, "13-explorer-alice-keys.png");

console.log("== the explorer, Bob's address with Bob's viewing key alone");
await open(`${WALLET}/explorer.html?address=bob#vk=${bobVk}`);
items = await feed();
await ok("the unshield payout to Bob is public, with its amount", items.some((t) => /privacy service paid Bob 15/.test(t)));
await ok("the viewing key opens Bob's 40", items.some((t) => /opens to this viewing key: 40/.test(t)));
await ok("the viewing key opens Bob's change, 25", items.some((t) => /opens to this viewing key: 25/.test(t)));
await ok("the viewing key cannot see spends", !items.some((t) => /spends this viewing key's note/.test(t)));
const sum = await text("summary");
const bSeen = privateBalance(sum);
await ok("Bob's viewing key counts what he received, spent or not: at least his 40 and 25", bSeen >= 65 && /spends need the spend key/i.test(sum), `${bSeen}`);
await show("Bob's viewing key · it opens his 40 and his change · it cannot spend them, or see spends", 7);
await shot(ex, "14-explorer-bob-viewing-key.png");

console.log(`PASS: ${pass} assertions`);
if (film) {
  await film.stop("explorer"); film.centre("explorer", 1.5);
  await film.titles("end");
  film.render(path.join(IMAGES, "demo.mp4"), path.join(IMAGES, "demo.gif"));
  console.log(`  film ${path.relative(process.cwd(), IMAGES)}/demo.mp4 and demo.gif`);
}
await browser.close();
