// #67: with PROVER_REAL_PROOFS=true (on-jam.sh's default) the node verifies each transaction's
// client proof (CHONK) before it enters the pool. Both ways, on one run:
//   1. a token transfer proves and mines (the node accepted a real proof);
//   2. a second transfer's proof, one field changed before submission, is refused by the node,
//      and that transfer's original is never sent.
// Exit 0 only if the mined transfer settles and the tampered one is refused.
//   node real-proofs-check.mjs [node url]       (default http://[::1]:8080)
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { getContractInstanceFromInstantiationParams } from '@aztec-labs/aztec.js/contracts';
import { SponsoredFeePaymentMethod } from '@aztec-labs/aztec.js/fee';
import { Fr } from '@aztec-labs/aztec.js/fields';
import { SPONSORED_FPC_SALT } from '@aztec-labs/constants';
import { SponsoredFPCContract } from '@aztec-labs/noir-contracts.js/SponsoredFPC';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';
import { getInitialTestAccountsData } from '@aztec-labs/accounts/testing';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';
import { createFundedInitializerlessAccounts } from '@aztec-labs/wallets/testing';

const real = createAztecNodeClient(process.argv[2] ?? 'http://[::1]:8080');
let onSendTx = null;                            // when set, takes the Tx instead of the node
const node = new Proxy(real, {
  get(target, key) {
    if (key === 'sendTx') return (tx) => (onSendTx ? onSendTx(tx) : target.sendTx(tx));
    const v = target[key];
    return typeof v === 'function' ? v.bind(target) : v;
  },
});

const wallet = await EmbeddedWallet.create(node, { pxe: { proverEnabled: true }, ephemeral: true });
const accounts = await getInitialTestAccountsData();
await createFundedInitializerlessAccounts(wallet, accounts);
const [admin, bob] = [accounts[0].address, accounts[1].address];
const wait = { timeout: 900, interval: 2 };
const fpc = await getContractInstanceFromInstantiationParams(SponsoredFPCContract.artifact, { salt: new Fr(SPONSORED_FPC_SALT) });
await wallet.registerContract(fpc, SponsoredFPCContract.artifact);
const fee = { paymentMethod: new SponsoredFeePaymentMethod(fpc.address) };
const say = (s) => console.log(s);

// 1. a real proof, accepted and mined
let t0 = Date.now();
const { contract: token } = await TokenContract.deploy(wallet, admin, 'JAM Token', 'JAMT', 6).send({ from: admin, fee, wait });
say(`token deployed with a real client proof, verified by the node: ${((Date.now() - t0) / 1000).toFixed(1)} s`);
t0 = Date.now();
const r = (await token.methods.mint_to_private(admin, 100n).send({ from: admin, fee, wait })).receipt;
say(`mint with a real client proof: status ${r.status}, block ${r.blockNumber}, ${((Date.now() - t0) / 1000).toFixed(1)} s`);

// 2. the same kind of transfer, its proof tampered: the node must refuse it
let refusal = '', acceptMs = 0;
onSendTx = async (tx) => {
  onSendTx = null;
  tx.chonkProof.fields[700] = Fr.random();      // one field of the client proof
  if ('compressedProof' in tx.chonkProof) tx.chonkProof.compressedProof = undefined;
  const t = Date.now();
  try {
    await real.sendTx(tx);
  } catch (e) {
    refusal = String(e.message ?? e);
    say(`tampered proof refused by the node in ${Date.now() - t} ms: ${refusal.slice(0, 100)}`);
    throw new Error('tampered-proof-refused');  // stop the wallet's wait; the original is never sent
  }
  acceptMs = Date.now() - t;
};
try {
  await token.methods.transfer(bob, 1n).send({ from: admin, fee, wait });
  say('FAIL: the tampered proof was accepted'); process.exit(1);
} catch (e) {
  if (!refusal) { say(`FAIL: ${e.message ?? e}`); process.exit(1); }
}

const balance = (await token.methods.balance_of_private(admin).simulate({ from: admin })).result;
await wallet.stop();
if (balance !== 100n) { say(`FAIL: admin holds ${balance}, expected 100 (the tampered transfer must not land)`); process.exit(1); }
say('PASS: real client proofs verified by the node; a tampered proof refused; the mint settled');
process.exit(0);
