// #53: an L2 transaction through Aztec v6's own PXE (EmbeddedWallet), to the node on JAM. As
// `aztec start --local-network` does at its start: register the funded test accounts, then publish
// the standard AuthRegistry, two transactions (its class, its instance), each waited on until
// mined. Everything here is Aztec's released code; publishStandardAuthRegistry repeats
// @aztec-labs/aztec's local-network/auth_registry.js, which the package doesn't export.
//   node send-tx.mjs <node url>
import { createAztecNodeClient } from '@aztec-labs/aztec.js/node';
import { publishContractClass, publishInstance } from '@aztec-labs/aztec.js/deployment';
import { AuthRegistryArtifact, getStandardAuthRegistry } from '@aztec-labs/standard-contracts/auth-registry';
import { getInitialTestAccountsData } from '@aztec-labs/accounts/testing';
import { EmbeddedWallet } from '@aztec-labs/wallets/embedded';
import { createFundedInitializerlessAccounts } from '@aztec-labs/wallets/testing';

const node = createAztecNodeClient(process.argv[2] ?? 'http://[::1]:8081');
const wallet = await EmbeddedWallet.create(node, { pxeConfig: { proverEnabled: false }, ephemeral: true });
const accounts = await getInitialTestAccountsData();
await createFundedInitializerlessAccounts(wallet, accounts);
const from = accounts[0].address;
const wait = { timeout: 900, interval: 2 };
const { instance, contractClass } = await getStandardAuthRegistry();
const t0 = Date.now();
const r1 = await (await publishContractClass(wallet, AuthRegistryArtifact)).send({ from, wait });
console.log(`publish class: tx ${r1.txHash ?? r1?.receipt?.txHash ?? ''} status ${r1.status ?? r1?.receipt?.status} block ${r1.blockNumber ?? r1?.receipt?.blockNumber} after ${((Date.now() - t0) / 1000).toFixed(0)} s`);
const r2 = await publishInstance(wallet, instance).send({ from, wait });
console.log(`publish instance: tx ${r2.txHash ?? r2?.receipt?.txHash ?? ''} status ${r2.status ?? r2?.receipt?.status} block ${r2.blockNumber ?? r2?.receipt?.blockNumber} after ${((Date.now() - t0) / 1000).toFixed(0)} s`);
console.log('class publicly registered:', (await wallet.getContractClassMetadata(contractClass.id)).isContractClassPubliclyRegistered,
  '| instance published:', (await wallet.getContractMetadata(instance.address)).isContractPublished);
await wallet.stop();
process.exit(0);
