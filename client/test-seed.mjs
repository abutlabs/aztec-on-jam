// client/seed.mjs against a fake indexer that applies each operation 2.5 s after it arrives,
// once per nonce as the public-balance service does, while two seeding runs go at the same moment
// and each resends what has not landed after a second. Neither run sees the other's mints in
// flight, so each must build the same operation as the other: one nonce per account and starting
// balance. Every account then ends with exactly the seeded amount, never twice it.
//   node client/test-seed.mjs
import http from "node:http";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const APPLY_MS = 2500;
const sends = new Map();          // account -> [mint payload hex, ...]
const balance = new Map(), keys = new Map(), seen = new Set();
const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://x"), parts = url.pathname.split("/").filter(Boolean);
  const json = (o) => { res.writeHead(200, { "content-type": "application/json" }); res.end(JSON.stringify(o)); };
  if (parts[0] === "state") return json({ public_id: 1 });
  if (parts[0] === "key") return json({ key: keys.get(parts[1]) ?? null });
  if (parts[0] === "balance") return json({ balance: String(balance.get(parts[1]) ?? 0n) });
  if (parts[0] === "submit") {
    let body = "";
    req.on("data", (c) => (body += c));
    req.on("end", () => {
      const op = parseInt(body.slice(0, 2), 16), account = body.slice(2, 66);
      if (op === 0) { const l = sends.get(account) ?? []; l.push(body); sends.set(account, l); }
      setTimeout(() => {                                   // lands later, applied once per nonce
        if (op === 3) keys.set(account, body.slice(66, 194));
        if (op === 0) {
          const nonce = body.slice(82, 146);
          if (seen.has(nonce)) return;
          seen.add(nonce);
          balance.set(account, (balance.get(account) ?? 0n) + Buffer.from(body.slice(66, 82), "hex").readBigUInt64LE());
        }
      }, APPLY_MS);
      json({ package_hash: "00".repeat(32), core: 0 });
    });
    return;
  }
  json({ error: "no such route" });
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const port = server.address().port;

const seed = path.join(path.dirname(fileURLToPath(import.meta.url)), "seed.mjs");
// asynchronously: the fake indexer answers from this same process
const runSeed = () => new Promise((ok) => {
  const c = spawn(process.execPath, ["--no-warnings", seed], {
    env: { ...process.env, INDEXER: `http://127.0.0.1:${port}`, SEED_WAIT_S: "1" } });
  let out = "";
  c.stdout.on("data", (d) => (out += d)); c.stderr.on("data", (d) => (out += d));
  c.on("close", (status) => ok({ status, out }));
});
const runs = await Promise.all([runSeed(), runSeed()]);
await new Promise((r) => setTimeout(r, APPLY_MS + 500));   // whatever is still in flight lands
server.close();

let fails = 0;
const check = (name, ok, detail = "") => { console.log(`  ${ok ? "ok  " : "FAIL"} ${name}${detail ? ": " + detail : ""}`); if (!ok) fails++; };
runs.forEach((r, i) => check(`run ${i + 1} finished`, r.status === 0, r.out.trim().split("\n").slice(-1)[0]));
check("six accounts minted", sends.size === 6, `${sends.size}`);
for (const [account, list] of sends) {
  const nonces = new Set(list.map((p) => p.slice(82, 146)));
  check(`${account.slice(0, 8)}…: ${list.length} mint sends, one operation`, nonces.size === 1, `${nonces.size} nonces`);
  check(`${account.slice(0, 8)}…: holds 1000, not more`, balance.get(account) === 1000n, `${balance.get(account)}`);
}
console.log(fails ? `FAIL: ${fails}` : "PASS: two seeding runs at once, every mint one operation, no account minted twice");
process.exit(fails ? 1 : 0);
