// Two private sends at the same moment, from two wallets in two browser contexts. Both prove
// from the same trees, so whichever accumulates second starts from roots the chain has left
// behind: accumulate refuses it, and resending the same bytes can never land. The page must see
// that its proof went stale, prove again from the new trees and land. Exit 0 only if both sends
// land, every balance is right, and at least one page proved a second time.
//   node client/e2e/race.mjs            (WALLET=http://localhost:19710, CHROME=system Chrome)
import puppeteer from "puppeteer-core";

const WALLET = process.env.WALLET || "http://localhost:19710";
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, defaultViewport: { width: 900, height: 1300 } });
const fail = async (msg) => { console.log("  FAIL " + msg); await browser.close(); process.exit(1); };
const status = (page) => page.$eval("#status", (e) => e.textContent);
const num = (page, id) => page.$eval("#" + id, (e) => Number(e.textContent));
const balances = async (page) => ({ public: await num(page, "public"), private: await num(page, "private") });
const set = (page, sel, v) => page.$eval(sel, (e, v) => { e.value = v; }, String(v));

async function open(user) {
  const page = await (await browser.createBrowserContext()).newPage();
  page.on("pageerror", (e) => console.log(`  [${user}] pageerror: ${String(e).slice(0, 200)}`));
  await page.goto(`${WALLET}/?user=${user}`);
  await page.waitForFunction(() => document.body.dataset.ready === "1" || document.getElementById("status").textContent.startsWith("error"), { timeout: 240000 });
  if ((await status(page)).startsWith("error")) await fail(`${user}: ${await status(page)}`);
  return page;
}
async function act(page, button) {
  await page.click(button);
  await page.waitForFunction(() => document.getElementById("status").textContent.startsWith("working"), { timeout: 10000 });
}
async function finish(page, name) {
  await page.waitForFunction(() => /^(done|error)/.test(document.getElementById("status").textContent), { timeout: 400000 });
  const s = await status(page);
  const steps = await page.$$eval("#steps li", (els) => els.map((e) => e.textContent));
  console.log(`  ${name}: ${s}`); for (const t of steps) console.log(`    ${t}`);
  if (s.startsWith("error")) await fail(`${name}: ${s}`);
  return steps;
}
async function settle(page, want) {
  for (let t0 = Date.now(); Date.now() - t0 < 30000; await new Promise((r) => setTimeout(r, 1000))) {
    const b = await balances(page);
    if (b.public === want.public && b.private === want.private) return b;
  }
  return balances(page);
}
function check(name, got, want) {
  const ok = got.public === want.public && got.private === want.private;
  console.log(`  ${ok ? "ok  " : "FAIL"} ${name}: public ${got.public} (want ${want.public}), private ${got.private} (want ${want.private})`);
  return ok ? null : fail(name);
}

const a = await open("A"), b = await open("B");
const aAddr = await a.$eval("#address", (e) => e.textContent.trim()), bAddr = await b.$eval("#address", (e) => e.textContent.trim());
console.log("== each wallet mints 100 and shields 100, one after the other");
for (const [page, name] of [[a, "A"], [b, "B"]]) {
  await act(page, "#btn-mint"); await finish(page, `${name} mint`);
  await set(page, "#amt-shield", 100); await act(page, "#btn-shield"); await finish(page, `${name} shield`);
}
const a0 = await balances(a), b0 = await balances(b);
console.log(`start: A ${JSON.stringify(a0)}, B ${JSON.stringify(b0)}`);

console.log("== at the same moment: A sends 30 to B, B sends 20 to A");
await set(a, "#amt-send", 30); await set(a, "#to-send", bAddr);
await set(b, "#amt-send", 20); await set(b, "#to-send", aAddr);
await Promise.all([act(a, "#btn-send"), act(b, "#btn-send")]);
const [sa, sb] = await Promise.all([finish(a, "A send"), finish(b, "B send")]);

await check("A after both", await settle(a, { public: a0.public, private: a0.private - 30 + 20 }), { public: a0.public, private: a0.private - 30 + 20 });
await check("B after both", await settle(b, { public: b0.public, private: b0.private - 20 + 30 }), { public: b0.public, private: b0.private - 20 + 30 });
const again = [...sa, ...sb].filter((t) => t.includes("proving again")).length;
if (again === 0) await fail("neither page proved again: both proofs landed from the same trees, so the race was not exercised (run it again)");
console.log(`PASS: both sends landed; ${again} stale proof(s) proved again`);
await browser.close();
