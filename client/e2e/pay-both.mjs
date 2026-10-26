// Two public payments at the same moment: Alice pays Bob 25 while Bob pays Alice 25, from two
// browser contexts. Each payment lands, but each payer's balance ends where it started, so a page
// that judged "landed" by its balance would never see its own debit and would report a failure.
// The page judges by its payment's nonce. Exit 0 only if both pages report the payment done and
// both public balances end unchanged. Needs the seeded dev accounts (nets/demo.sh up).
//   node client/e2e/pay-both.mjs        (WALLET=http://localhost:19710, CHROME=system Chrome)
import puppeteer from "puppeteer-core";

const WALLET = process.env.WALLET || "http://localhost:19710";
const INDEXER = process.env.INDEXER || "http://localhost:19700";
const transfers = async () => (await (await fetch(`${INDEXER}/history`)).json()).events.filter((e) => e.kind === "transfer");
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, defaultViewport: { width: 900, height: 1300 } });
const fail = async (msg) => { console.log("  FAIL " + msg); await browser.close(); process.exit(1); };
const status = (page) => page.$eval("#status", (e) => e.textContent);
const num = (page, id) => page.$eval("#" + id, (e) => Number(e.textContent));
const set = (page, sel, v) => page.$eval(sel, (e, v) => { e.value = v; }, String(v));

async function open(user) {
  const page = await (await browser.createBrowserContext()).newPage();
  page.on("pageerror", (e) => console.log(`  [${user}] pageerror: ${String(e).slice(0, 200)}`));
  await page.goto(`${WALLET}/?user=${user}`);
  await page.waitForFunction(() => document.body.dataset.ready === "1" || document.getElementById("status").textContent.startsWith("error"), { timeout: 240000 });
  if ((await status(page)).startsWith("error")) await fail(`${user}: ${await status(page)}`);
  return page;
}
async function pay(page, to) {
  await set(page, "#to-pay", to); await set(page, "#amt-pay", 25);
  await page.click("#btn-pay");
  await page.waitForFunction(() => document.getElementById("status").textContent.startsWith("working"), { timeout: 10000 });
}
async function finish(page, name) {
  await page.waitForFunction(() => /^(done|error)/.test(document.getElementById("status").textContent), { timeout: 400000 });
  const s = await status(page);
  console.log(`  ${name}: ${s}`);
  return s;
}

const alice = await open("alice"), bob = await open("bob");
const a0 = await num(alice, "public"), b0 = await num(bob, "public");
const aAddr = await alice.$eval("#address", (e) => e.textContent.trim()), bAddr = await bob.$eval("#address", (e) => e.textContent.trim());
const seenBefore = (await transfers()).length;
console.log(`start: Alice ${a0}, Bob ${b0} public`);
if (a0 < 25 || b0 < 25) await fail("Alice and Bob need 25 public each: run nets/demo.sh seed");

console.log("== at the same moment: Alice pays Bob 25, Bob pays Alice 25");
await Promise.all([pay(alice, "bob"), pay(bob, "alice")]);
const [sa, sb] = await Promise.all([finish(alice, "Alice"), finish(bob, "Bob")]);
if (!sa.startsWith("done")) await fail(`Alice's payment: ${sa}`);
if (!sb.startsWith("done")) await fail(`Bob's payment: ${sb}`);

// both landed: each balance is back where it started
for (let t0 = Date.now(); Date.now() - t0 < 30000; await new Promise((r) => setTimeout(r, 1000))) {
  if (await num(alice, "public") === a0 && await num(bob, "public") === b0) break;
}
// and the chain's own log has both, once each
let landed = [];
for (let t0 = Date.now(); Date.now() - t0 < 30000; await new Promise((r) => setTimeout(r, 1000))) {
  landed = (await transfers()).slice(seenBefore);
  if (landed.length >= 2) break;
}
const ab = landed.filter((e) => e.account === aAddr && e.to === bAddr && e.amount === 25).length;
const ba = landed.filter((e) => e.account === bAddr && e.to === aAddr && e.amount === 25).length;
if (ab !== 1 || ba !== 1) await fail(`the chain's log: Alice->Bob ${ab}, Bob->Alice ${ba} (want 1 each); ${landed.length} new transfers`);
console.log("  ok   the chain's log has Alice->Bob 25 and Bob->Alice 25, once each");
const a1 = await num(alice, "public"), b1 = await num(bob, "public");
if (a1 !== a0 || b1 !== b0) await fail(`balances moved: Alice ${a0} -> ${a1}, Bob ${b0} -> ${b1}`);
console.log(`  ok   both payments done, Alice ${a1} and Bob ${b1} public, as before`);
console.log("PASS: two crossing payments both reported done");
await browser.close();
