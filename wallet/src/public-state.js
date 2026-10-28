// What anyone can read of a Token: its public storage, through Aztec's node, with no wallet and
// no keys. The slots are the contract's own storage layout (Aztec's Token, as compiled).
import { Fr } from '@aztec-labs/aztec.js/fields';
import { deriveStorageSlotInMap } from '@aztec-labs/stdlib/hash';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';

const read = async (node, token, slot) => (await node.getPublicStorageAt('latest', token, slot)).toBigInt();

/** `owner`'s public balance. Private balances are stored nowhere in the clear. */
export async function publicBalance(node, token, owner) {
  return read(node, token, await deriveStorageSlotInMap(TokenContract.storage.public_balances.slot, owner));
}

/** Every token minted, public and private: Token keeps the total in public state. */
export async function totalSupply(node, token) {
  return read(node, token, new Fr(TokenContract.storage.total_supply.slot.toBigInt()));
}
