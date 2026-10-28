// The explorer's private view, checked in Node against a running stack. Alice mints, unshields
// and sends Bob 40; each account's view-keys bundle (src/view-keys.js), passed as text, is
// imported into a fresh PXE that never held the account. That PXE must read the true balances
// (spent notes counted) and the Transfer history, and must not be able to send. Public balances
// are also read with no keys at all, straight from public state, and must agree.
//   node view-keys.mjs [node url]       (default http://localhost:8080; Node 22+)
if (Number(process.versions.node.split('.')[0]) < 22) { console.error(`Node 22+ is needed (this is ${process.versions.node})`); process.exit(2); }
const { createAztecNodeClient } = await import('@aztec-labs/aztec.js/node');
const { getContractInstanceFromInstantiationParams } = await import('@aztec-labs/aztec.js/contracts');
const { SponsoredFeePaymentMethod } = await import('@aztec-labs/aztec.js/fee');
const { Fr, Fq } = await import('@aztec-labs/aztec.js/fields');
const { SPONSORED_FPC_SALT } = await import('@aztec-labs/constants');
const { SponsoredFPCContract } = await import('@aztec-labs/noir-contracts.js/SponsoredFPC');
const { TokenContract } = await import('@aztec-labs/noir-contracts.js/Token');
const { EmbeddedWallet } = await import('@aztec-labs/wallets/embedded');
const { exportViewKeys, parseViewKeys, importViewKeys } = await import('./src/view-keys.js');
const { publicBalance, totalSupply } = await import('./src/public-state.js');

const node = createAztecNodeClient(process.argv[2] ?? 'http://localhost:8080');
const wait = { timeout: 900, interval: 2 };
const t0 = Date.now();
const at = () => `${((Date.now() - t0) / 1000).toFixed(0)} s`;
let failed = 0;
const ok = (name, cond, detail = '') => { console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${name}${detail ? ': ' + detail : ''}`); if (!cond) failed++; };

// two wallets, as two people would have: fees through the genesis-funded SponsoredFPC
const fpc = await getContractInstanceFromInstantiationParams(SponsoredFPCContract.artifact, { salt: new Fr(SPONSORED_FPC_SALT) });
const fee = { paymentMethod: new SponsoredFeePaymentMethod(fpc.address) };
const open = async (prover) => {
  const w = await EmbeddedWallet.create(node, { pxeConfig: { proverEnabled: prover }, ephemeral: true });
  await w.registerContract(fpc, SponsoredFPCContract.artifact);
  return w;
};
const aw = await open(true), bw = await open(true);
const alice = (await aw.createSchnorrInitializerlessAccount(Fr.random(), Fr.random(), Fq.random(), 'alice')).address;
const bob = (await bw.createSchnorrInitializerlessAccount(Fr.random(), Fr.random(), Fq.random(), 'bob')).address;
await bw.registerSender(alice, 'alice');

const { contract: token } = await TokenContract.deploy(aw, alice, 'Rebel Token', 'RBL', 6).send({ from: alice, fee, wait });
await token.methods.mint_to_private(alice, 1000n).send({ from: alice, fee, wait });
await token.methods.transfer_to_public(alice, alice, 100n, 0).send({ from: alice, fee, wait });
const { receipt: sent } = await token.methods.transfer(bob, 40n).send({ from: alice, fee, wait });
console.log(`alice ${alice}\nbob   ${bob}\ntoken ${token.address}: alice minted 1000, unshielded 100, sent bob 40 in block ${sent.blockNumber} (${at()})`);

// the bundles, as the wallet shows them: text, then parsed back
const aliceText = await exportViewKeys(aw, alice, { token: token.address });
const bobText = await exportViewKeys(bw, bob, { token: token.address });
ok('a bundle holds no account secret and no signing key',
  ![(await aw.walletDB.retrieveAccount(alice)).secretKey.toString(), Buffer.from((await aw.walletDB.retrieveAccount(alice)).signingKey).toString('hex')]
    .some((s) => aliceText.includes(s.replace(/^0x/, ''))));
ok('the bundle\'s secret keys are the ones PXE itself exports',
  Object.entries(await aw.pxe.getAccountSecretKeys(alice)).every(([k, v]) => parseViewKeys(aliceText).keys[k].equals(v)));
ok('bob\'s bundle lists alice as a sender', parseViewKeys(bobText).senders.some((s) => s.equals(alice)));
let bad = '';
try { parseViewKeys('{"kind":"something else"}'); } catch (e) { bad = e.message; }
ok('a wrong paste is refused with a reason', /not a view-keys bundle/.test(bad), bad);

// the viewer: a fresh PXE, no account of its own, no prover
const viewer = await open(false);
await viewer.registerContract(await node.getContract(token.address), TokenContract.artifact);
const seen = await TokenContract.at(token.address, viewer);
for (const [name, text, priv, pub] of [['alice', aliceText, 860n, 100n], ['bob', bobText, 40n, 0n]]) {
  const who = await importViewKeys(viewer, parseViewKeys(text));
  const p = (await seen.methods.balance_of_private(who).simulate({ from: who })).result;
  ok(`${name}'s keys read the private balance, spends counted`, p === priv, `${p}`);
  const q = await publicBalance(node, token.address, who);
  ok(`${name}'s public balance, read with no keys, agrees with the contract`, q === pub && q === (await seen.methods.balance_of_public(who).simulate({ from: who })).result, `${q}`);
}
const supply = await totalSupply(node, token.address);
ok('total supply is public, though who holds it is not', supply === 1000n, `${supply}`);
const events = await viewer.getPrivateEvents(TokenContract.events.Transfer, { contractAddress: token.address, scopes: [bob] });
const fromAlice = events.find((e) => e.event.amount === 40n);
ok('bob\'s keys reveal the 40 from alice, with its tx', fromAlice && fromAlice.metadata.txHash.equals(sent.txHash)
  && String(fromAlice.event.from) === alice.toString(), events.map((e) => `${e.event.amount} ${String(e.event.from).slice(0, 10)}→${String(e.event.to).slice(0, 10)}`).join(', '));
const aliceEvents = await viewer.getPrivateEvents(TokenContract.events.Transfer, { contractAddress: token.address, scopes: [alice] });
console.log(`  (alice's keys reveal ${aliceEvents.length} Transfer events: ${aliceEvents.map((e) => `${e.event.amount} ${String(e.event.from).slice(0, 10)}→${String(e.event.to).slice(0, 10)}`).join(', ')})`);

let refused = '';
try { await seen.methods.transfer(alice, 1n).send({ from: bob, fee, wait }); } catch (e) { refused = String(e.message ?? e).slice(0, 140); }
ok('the viewer cannot send as bob', refused !== '', refused);

await Promise.all([aw.stop(), bw.stop(), viewer.stop()]);
console.log(failed ? `FAIL: ${failed} (${at()})` : `PASS (${at()})`);
process.exit(failed ? 1 : 0);
