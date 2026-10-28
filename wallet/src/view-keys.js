// What the wallet hands the explorer so it can see an account's private activity, and nothing
// that could spend. Aztec splits an account's keys: the Schnorr signing key authorises
// transactions and never leaves the wallet; the account secret derives the four privacy keys
// (nullifier-hiding, incoming-viewing, outgoing-viewing, tagging) and two keys PXE never holds
// (message-signing, fallback). Aztec's PXE.registerAccount takes the four privacy secret keys and
// the two public keys: per Aztec, these "do NOT grant control over an account's assets" but do
// reveal all of its activity. The bundle carries exactly those, the address they must rebuild,
// and the wallet's senders (notes are found by tags shared with known senders).
import { AztecAddress } from '@aztec-labs/aztec.js/addresses';
import { Fr, GrumpkinScalar, Point } from '@aztec-labs/aztec.js/fields';
import { deriveKeys } from '@aztec-labs/aztec.js/keys';

export const KIND = 'aztec-view-keys/1';
const SECRET = ['masterNullifierHidingSecretKey', 'masterIncomingViewingSecretKey', 'masterOutgoingViewingSecretKey', 'masterTaggingSecretKey'];
const PUBLIC = ['masterMessageSigningPublicKey', 'masterFallbackPublicKey'];

/** The bundle for `address`, built in the wallet that holds it (EmbeddedWallet), as JSON text. */
export async function exportViewKeys(wallet, address, { token } = {}) {
  const { secretKey } = await wallet.walletDB.retrieveAccount(address);
  const keys = await deriveKeys(secretKey);
  const complete = (await wallet.pxe.getRegisteredAccounts()).find((a) => a.address.equals(address));
  const out = { kind: KIND, address: address.toString(), partialAddress: complete.partialAddress.toString() };
  for (const k of [...SECRET, ...PUBLIC]) out[k] = keys[k].toString();
  out.senders = (await wallet.getAddressBook()).map((s) => s.item.toString());
  if (token) out.token = token.toString();
  return JSON.stringify(out, null, 1);
}

/** Parses pasted bundle text; throws a plain reason when it is not one. */
export function parseViewKeys(text) {
  let o;
  try { o = JSON.parse(text); } catch { throw new Error('that is not a view-keys bundle (it is not JSON)'); }
  if (o?.kind !== KIND) throw new Error(`that is not a view-keys bundle (expected kind "${KIND}")`);
  const missing = ['address', 'partialAddress', ...SECRET, ...PUBLIC].filter((k) => typeof o[k] !== 'string');
  if (missing.length) throw new Error(`the bundle is missing ${missing.join(', ')}`);
  const keys = {};
  for (const k of SECRET) keys[k] = GrumpkinScalar.fromString(o[k]);
  for (const k of PUBLIC) keys[k] = Point.fromString(o[k]);
  return {
    address: AztecAddress.fromStringUnsafe(o.address),
    partialAddress: Fr.fromString(o.partialAddress),
    keys,
    senders: (o.senders ?? []).map((s) => AztecAddress.fromStringUnsafe(s)),
    token: o.token ? AztecAddress.fromStringUnsafe(o.token) : undefined,
  };
}

/** Registers a parsed bundle's account, view only, in `wallet`'s PXE, and its senders. */
export async function importViewKeys(wallet, { address, partialAddress, keys, senders }) {
  const complete = await wallet.pxe.registerAccount(keys, partialAddress);
  if (!complete.address.equals(address)) throw new Error('these keys do not belong to that address');
  for (const s of senders) await wallet.registerSender(s, `sender ${s.toString().slice(0, 10)}`);
  return complete.address;
}
