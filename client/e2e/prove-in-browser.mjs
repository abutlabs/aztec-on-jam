// Proves a transfer inside headless Chrome with the wallet page's bundled provers
// (demo-wallet jam/dist/provers.js): noir_js for the witness, bb.js for the proof.
//   node client/e2e/prove-in-browser.mjs <url of jam/prove-test.html>
import puppeteer from "puppeteer-core";
const url = process.argv[2] || "http://127.0.0.1:19720/prove-test.html";
const b = await puppeteer.launch({ executablePath: process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", headless: true });
const p = await b.newPage();
p.on("console", (m) => console.log("page:", m.text().slice(0, 300)));
p.on("pageerror", (e) => console.log("pageerror:", String(e).slice(0, 300)));
await p.goto(url);
await p.waitForFunction(() => document.title === "PROVED" || document.title === "FAILED", { timeout: 300000 });
const title = await p.title();
console.log("title:", title);
await b.close();
process.exit(title === "PROVED" ? 0 : 1);
