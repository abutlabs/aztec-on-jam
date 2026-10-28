// #53, #64: L2 transactions through Aztec v6's own PXE (EmbeddedWallet), to the node on JAM.
// Everything here is Aztec's released code. The story is #64's check:
//   1. a funded test account deploys Aztec's Token and mints 1000 to a fresh account;
//   2. the fresh account — initializerless Schnorr, so never deployed, and holding no fee
//      juice — transfers 40 back, its fee paid by the genesis-funded SponsoredFPC
//      (SponsoredFeePaymentMethod);
//   3. both private balances are read back and asserted.
// Exit 0 only if every transaction mines and the balances are 960 and 40.
//   node send-tx.mjs [node url]       (default http://[::1]:8080)
import './node-check.mjs';
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { getContractInstanceFromInstantiationParams } from '@aztec-labs/aztec.js/contracts';
import { SponsoredFeePaymentMethod } from '@aztec-labs/aztec.js/fee';
import { Fr, Fq } from '@aztec-labs/aztec.js/fields';
import { SPONSORED_FPC_SALT } from '@aztec-labs/constants';
import { SponsoredFPCContract } from '@aztec-labs/noir-contracts.js/SponsoredFPC';
import { TokenContract } from '@aztec-labs/noir-contracts.js/Token';
import { getInitialTestAccountsData } from '@aztec-labs/accounts/testing';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';
import { createFundedInitializerlessAccounts } from '@aztec-labs/wallets/testing';

const node = createAztecNodeClient(process.argv[2] ?? 'http://[::1]:8080');
// a transaction built before block 1 is valid only in block 1: wait for the chain to start
while ((await node.getBlockNumber()) < 1) await new Promise((r) => setTimeout(r, 2000));
const wallet = await EmbeddedWallet.create(node, { pxeConfig: { proverEnabled: true }, ephemeral: true });  // the node verifies real proofs (#67)
const accounts = await getInitialTestAccountsData();
await createFundedInitializerlessAccounts(wallet, accounts);
const admin = accounts[0].address;
const wait = { timeout: 900, interval: 2 };
const t0 = Date.now();
const log = (what, r) => { r = r.receipt ?? r; console.log(`${what}: tx ${r.txHash} status ${r.status} block ${r.blockNumber} after ${((Date.now() - t0) / 1000).toFixed(0)} s`); };

// the fresh account: initializerless, so creating it registers it locally with no deployment
const fresh = (await wallet.createSchnorrInitializerlessAccount(Fr.random(), Fr.random(), Fq.random(), 'fresh')).address;
// the SponsoredFPC: genesis funds it at its deterministic address (salt 0); registering the
// instance with the PXE is all a user needs to pay through it
const fpc = await getContractInstanceFromInstantiationParams(SponsoredFPCContract.artifact, { salt: new Fr(SPONSORED_FPC_SALT) });
await wallet.registerContract(fpc, SponsoredFPCContract.artifact);
console.log(`fresh account ${fresh}; SponsoredFPC ${fpc.address}`);

const { contract: token, receipt: rd } = await TokenContract.deploy(wallet, admin, 'JAM Token', 'JAMT', 6).send({ from: admin, wait });
log(`token deployed at ${token.address} (admin pays with its own fee juice)`, rd);
log('admin mints 1000 to fresh, privately', await token.methods.mint_to_private(fresh, 1000n).send({ from: admin, wait }));
log('fresh transfers 40 to admin, fee paid by the SponsoredFPC',
  await token.methods.transfer(admin, 40n).send({ from: fresh, fee: { paymentMethod: new SponsoredFeePaymentMethod(fpc.address) }, wait }));

const balance = async (who) => (await token.methods.balance_of_private(who).simulate({ from: who })).result;
const [bFresh, bAdmin] = [await balance(fresh), await balance(admin)];
console.log(`private balances: fresh ${bFresh}, admin ${bAdmin}`);
await wallet.stop();
if (bFresh !== 960n || bAdmin !== 40n) { console.log('FAIL: expected 960 and 40'); process.exit(1); }
console.log('PASS: a never-deployed, unfunded account transacted with a sponsored fee, settled on JAM');
process.exit(0);
