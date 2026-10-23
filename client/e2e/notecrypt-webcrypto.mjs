// Cross-check: client/notecrypt.py seals a note; WebCrypto (Node's, the browser's API) opens it.
// The browser wallet must open exactly what the Python client seals, and the reverse.
//   node client/e2e/notecrypt-webcrypto.mjs
import { webcrypto as c } from "node:crypto";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const CLIENT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const py = (code) => execFileSync("python3", ["-c", `import sys; sys.path.insert(0, ${JSON.stringify(CLIENT)}); import notecrypt as n\n${code}`]).toString().trim();
const h = (s) => Uint8Array.from(Buffer.from(s, "hex"));
const INFO = new TextEncoder().encode("jam-privacy-note-v1");

async function x25519Keys(skHex) {
  const pkcs8 = Uint8Array.from([...h("302e020100300506032b656e04220420"), ...h(skHex)]);
  return c.subtle.importKey("pkcs8", pkcs8, { name: "X25519" }, false, ["deriveBits"]);
}

async function keysFor(priv, epk) {
  const pub = await c.subtle.importKey("raw", epk, { name: "X25519" }, false, []);
  const secret = new Uint8Array(await c.subtle.deriveBits({ name: "X25519", public: pub }, priv, 256));
  const hk = await c.subtle.importKey("raw", secret, "HKDF", false, ["deriveBits"]);
  return new Uint8Array(await c.subtle.deriveBits({ name: "HKDF", hash: "SHA-256", salt: epk, info: INFO }, hk, 512));
}

async function keystream(key, n) {
  const out = new Uint8Array(Math.ceil(n / 32) * 32);
  for (let i = 0; i * 32 < n; i++) {
    const ctr = new Uint8Array(4); new DataView(ctr.buffer).setUint32(0, i);
    out.set(new Uint8Array(await c.subtle.digest("SHA-256", Uint8Array.from([...key, ...ctr]))), i * 32);
  }
  return out.slice(0, n);
}

async function open(priv, sealed) {
  const epk = sealed.slice(0, 32), ct = sealed.slice(32, -16), tag = sealed.slice(-16);
  const keys = await keysFor(priv, epk);
  const mk = await c.subtle.importKey("raw", keys.slice(32), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const mac = new Uint8Array(await c.subtle.sign("HMAC", mk, Uint8Array.from([...epk, ...ct]))).slice(0, 16);
  if (Buffer.compare(Buffer.from(mac), Buffer.from(tag)) !== 0) return null;
  const ks = await keystream(keys.slice(0, 32), ct.length);
  return ct.map((b, i) => b ^ ks[i]);
}

const [skHex, pkHex, sealedHex] = py(`sk, pk = n.keypair(b"B" * 32)\ns = n.seal(pk, n.note_plaintext(40, 777))\nprint(sk.hex(), pk.hex(), s.hex())`).split(" ");
const priv = await x25519Keys(skHex);
const pt = await open(priv, h(sealedHex));
if (!pt) throw new Error("WebCrypto could not open the Python-sealed note");
const value = Buffer.from(pt.slice(0, 8)).readBigUInt64LE(0), blinding = BigInt("0x" + Buffer.from(pt.slice(8, 40)).toString("hex"));
if (value !== 40n || blinding !== 777n) throw new Error(`wrong plaintext: ${value} ${blinding}`);
const tampered = h(sealedHex); tampered[40] ^= 1;
if (await open(priv, tampered)) throw new Error("a tampered note opened");
// the public key WebCrypto derives from the same secret matches Python's
const kp = await c.subtle.importKey("pkcs8", Uint8Array.from([...h("302e020100300506032b656e04220420"), ...h(skHex)]), { name: "X25519" }, true, ["deriveBits"]);
const jwk = await c.subtle.exportKey("jwk", kp);
if (Buffer.from(jwk.x, "base64url").toString("hex") !== pkHex) throw new Error("public keys differ");
console.log("notecrypt cross-check: WebCrypto opens the Python-sealed note (value 40, blinding 777), refuses a tampered one, and derives the same public key");
