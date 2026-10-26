// Seeds a fresh net with the standard JAM dev accounts (JIP-5): Alice, Bob, Carol, David, Eve
// and Fergie each publish their address in the directory and hold SEED public balance from the
// faucet. Their wallets derive from their public seeds (../demo-wallet/jam/lib/core.js), so any
// browser opens them at ?user=alice and the explorer can name them. Idempotent: an address
// already published or a balance already at SEED is left alone.
//   node client/seed.mjs            (INDEXER=http://localhost:19700, SEED=1000)
import * as W from "../../demo-wallet/jam/lib/core.js";

const INDEXER = (process.env.INDEXER || "http://localhost:19700").replace(/\/$/, "");
const SEED = BigInt(process.env.SEED || 1000);
const WAIT_S = 60, TRIES = 3;
const get = async (path) => { const r = await fetch(INDEXER + path); const j = await r.json(); if (!r.ok) throw new Error(j.error || `${path}: ${r.status}`); return j; };
async function submit(service, bytes, core) {
  const r = await fetch(`${INDEXER}/submit/${service}?core=${core}`, { method: "POST", body: W.hex(bytes) });
  const j = await r.json(); if (!r.ok || j.error) throw new Error(j.error || `submit: ${r.status}`); return j;
}

await W.init();
const state = await get("/state");
const accounts = [];
for (let i = 0; i < W.DEV_ACCOUNTS.length; i++) {
  const id = await W.identity(W.devSecret(i));
  accounts.push({ name: W.DEV_ACCOUNTS[i], id, entry: W.directoryEntry(id) });
}

// every operation still needed, as [label, landed(), make payload]: all sent at once, spread over
// the tiny net's two cores, each sent again (a fresh nonce, so never twice applied) if not landed
const jobs = [];
for (const a of accounts) {
  jobs.push([`${a.name}: address`, async () => (await get(`/key/${a.id.account}`)).key === a.entry, () => W.opKey(a.id.account, a.entry)]);
  jobs.push([`${a.name}: ${SEED} public`, async () => BigInt((await get(`/balance/${a.id.account}`)).balance) >= SEED,
    async () => W.opMint(a.id.account, SEED - BigInt((await get(`/balance/${a.id.account}`)).balance))]);
}
let pending = [];
for (const j of jobs) if (!(await j[1]())) pending.push(j);
console.log(`seed: ${jobs.length - pending.length} of ${jobs.length} already on chain`);
for (let attempt = 1; attempt <= TRIES && pending.length; attempt++) {
  await Promise.all(pending.map(async ([label, , make], k) => {
    const r = await submit(state.public_id, await make(), k % 2);
    console.log(`  ${label}: package ${r.package_hash.slice(0, 12)}… on core ${r.core}${attempt > 1 ? ` (resend ${attempt - 1})` : ""}`);
  }));
  const t0 = Date.now();
  while (pending.length && Date.now() - t0 < WAIT_S * 1000) {
    await new Promise((ok) => setTimeout(ok, 3000));
    const still = [];
    for (const j of pending) if (!(await j[1]())) still.push(j);
    pending = still;
  }
}
if (pending.length) { console.log(`seed: FAIL, not on chain: ${pending.map((j) => j[0]).join(", ")}`); process.exit(1); }
for (const a of accounts) console.log(`  ${a.name.padEnd(7)} ${a.id.account}  public ${(await get(`/balance/${a.id.account}`)).balance}`);
console.log("seed: every dev account is published and funded");
