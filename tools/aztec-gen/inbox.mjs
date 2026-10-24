// L1→L2 message leaves and the Inbox's rolling hash, by Aztec v6's released code
// (@aztec-labs/stdlib 6.0.0: L1ToL2Message.hash, accumulateInboxRollingHash), unchanged, for the
// settlement service's Inbox (#50). On JAM a message's L1 sender is the sending service: its id,
// as a 20-byte address, on the settlement service's chain id.
//   node inbox.mjs <spec.json>: spec {chainId, version, messages: [{sender, recipient, content,
//   secretHash}], buckets: [count, ...]} (all numbers or 0x-hex strings). Prints JSON: each
//   message's leaf, and the rolling hash and total after each bucket.
import { L1ToL2Message, L1Actor, L2Actor, accumulateInboxRollingHash } from '@aztec-labs/stdlib/messaging';
import { AztecAddress } from '@aztec-labs/stdlib/aztec-address';
import { EthAddress } from '@aztec-labs/foundation/eth-address';
import { Fr } from '@aztec-labs/foundation/curves/bn254';
import { readFileSync } from 'fs';

const spec = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const fr = x => new Fr(BigInt(x));
const leaves = spec.messages.map((m, index) =>
  new L1ToL2Message(
    new L1Actor(EthAddress.fromString('0x' + BigInt(m.sender).toString(16).padStart(40, '0')), Number(spec.chainId)),
    new L2Actor(AztecAddress.fromBigIntUnsafe(BigInt(m.recipient)), Number(spec.version)),
    fr(m.content), fr(m.secretHash), fr(index),
  ).hash());
let rolling = Fr.ZERO, total = 0;
const buckets = spec.buckets.map(count => {
  rolling = accumulateInboxRollingHash(rolling, leaves.slice(total, total + count));
  total += count;
  return { rollingHash: rolling.toString(), total };
});
console.log(JSON.stringify({ leaves: leaves.map(l => l.toString()), buckets }, null, 1));
process.exit(0);
