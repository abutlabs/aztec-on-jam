// Seeds a fresh net with the standard JAM dev accounts (JIP-5): Alice, Bob, Carol, David, Eve
// and Fergie each publish their address in the directory and hold SEED public balance from the
// faucet. Their wallets derive from their public seeds (../demo-wallet/jam/lib/core.js), so any
// browser opens them at ?user=alice and the explorer can name them. Idempotent: an address
// already published or a balance already at SEED is left alone.
//   node client/seed.mjs            (INDEXER=http://localhost:19700, SEED=1000, SEED_WAIT_S=60)
import { createHash } from "node:crypto";
import * as W from "../../demo-wallet/jam/lib/core.js";

const INDEXER = (process.env.INDEXER || "http://localhost:19700").replace(/\/$/, "");
const SEED = BigInt(process.env.SEED || 1000);
const WAIT_S = Number(process.env.SEED_WAIT_S || 60), TRIES = 3;   // seconds an attempt waits to land
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

// every operation still needed: all sent at once, spread over the tiny net's two cores. Each
// payload is built once, and a resend sends the same bytes. A mint's nonce comes from the account
// and the balance it tops up from, so a slow first package and its resend, or two seeding runs
// at once, build the same operation and the service applies it once; a later top-up from another
// balance is a new one (publishing a key is idempotent anyway)
const seedNonce = (account, balance) =>
  createHash("sha256").update("jam-privacy-seed:").update(Buffer.from(account, "hex")).update(W.u64le(balance)).digest();
const jobs = [];
for (const a of accounts) {
  jobs.push({ label: `${a.name}: address`, landed: async () => (await get(`/key/${a.id.account}`)).key === a.entry,
    make: async () => W.opKey(a.id.account, a.entry) });
  jobs.push({ label: `${a.name}: ${SEED} public`, landed: async () => BigInt((await get(`/balance/${a.id.account}`)).balance) >= SEED,
    make: async () => {
      const bal = BigInt((await get(`/balance/${a.id.account}`)).balance);
      return W.opMint(a.id.account, SEED - bal, seedNonce(a.id.account, bal));
    } });
}
let pending = [];
for (const j of jobs) if (!(await j.landed())) pending.push(j);
console.log(`seed: ${jobs.length - pending.length} of ${jobs.length} already on chain`);
for (let attempt = 1; attempt <= TRIES && pending.length; attempt++) {
  await Promise.all(pending.map(async (j, k) => {
    j.payload ??= await j.make();
    const r = await submit(state.public_id, j.payload, k % 2);
    console.log(`  ${j.label}: package ${r.package_hash.slice(0, 12)}… on core ${r.core}${attempt > 1 ? ` (resend ${attempt - 1})` : ""}`);
  }));
  const t0 = Date.now();
  while (pending.length && Date.now() - t0 < WAIT_S * 1000) {
    await new Promise((ok) => setTimeout(ok, 3000));
    const still = [];
    for (const j of pending) if (!(await j.landed())) still.push(j);
    pending = still;
  }
}
if (pending.length) { console.log(`seed: FAIL, not on chain: ${pending.map((j) => j.label).join(", ")}`); process.exit(1); }
for (const a of accounts) console.log(`  ${a.name.padEnd(7)} ${a.id.account}  public ${(await get(`/balance/${a.id.account}`)).balance}`);
console.log("seed: every dev account is published and funded");
