#!/usr/bin/env bash
# #53: Aztec v6's released node on JAM. A PolkaJam devnet with the settlement service configured
# for the node's own genesis (services/aztec-settlement/devnet.sh), jam-l1.mjs as the node's L1,
# then the node: `aztec start` with the roles given (default --node), test accounts prefunded,
# P2P off, blobs through a file store the sequencer writes and the archiver reads, and test-mode
# proofs as Aztec's local network uses them.
#   on-jam.sh [aztec start flags...]       stop with Ctrl-C; everything it started stops
# Its out dir is ../../out/aztec-devnet: devnet.log, jam-l1.log (every L1 request, "unhandled"
# marks a gap), node.log. The node answers on NODE_PORT (default 8080, where a wallet built for
# Aztec's local network looks). Genesis funds the test accounts and the SponsoredFPC (#64), so a
# fresh account can pay fees through SponsoredFeePaymentMethod.
set -euo pipefail
# Aztec v6's packages need Node 22+ (global WebSocket; the facade uses it too). Prefer the
# newest nvm node when the shell's is older.
if [ "$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)" -lt 22 ]; then
  for v in $(ls -d "$HOME"/.nvm/versions/node/v* 2>/dev/null | sort -V); do PATH="$v/bin:$PATH"; done
  export PATH
  [ "$(node -p 'process.versions.node.split(".")[0]')" -ge 22 ] || { echo "on-jam.sh: Node 22+ needed" >&2; exit 2; }
fi
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); D=$ROOT/out/aztec-devnet
# Aztec v6's genesis: the test accounts and the SponsoredFPC funded (TEST_ACCOUNTS=true,
# SPONSORED_FPC=true; computed by the node's own getGenesisValues, #64), and its circuits
GENESIS_ARCHIVE=0x0ad1dcc0e08d2b1c82a79af5685598e631e881187c3c9734ad28df5dbcf5db48
VK_TREE_ROOT=0x22fff5de6ce590153df4468f7f0b152188a803d91b3c9c648b91d93d5913f219
PROTOCOL_CONTRACTS_HASH=0x0f54271c52865841a77aaa66036eed901aff22fdce9f08e123ec8b205c9854d7
REGISTRY=0x00000000000000000000000000000000a11ce001
rm -rf "$D"; mkdir -p "$D/blobs" "$D/node-data"
pids=()
trap 'kill "${pids[@]}" 2>/dev/null || true; wait 2>/dev/null || true' EXIT INT TERM
"$ROOT/services/aztec-settlement/devnet.sh" "$D" "$GENESIS_ARCHIVE" "$VK_TREE_ROOT" "$PROTOCOL_CONTRACTS_HASH" > "$D/devnet.log" 2>&1 & pids+=($!)
until grep -qE "devnet up|rror|never finalized" "$D/devnet.log"; do sleep 2; done
grep -q "devnet up" "$D/devnet.log" || { tail -20 "$D/devnet.log"; exit 1; }
python3 - "$D/devnet.json" "$D/jam-l1.json" "$D" "$ROOT/../jamboree/target/release/jamboree" <<'PY'
import json, sys
d, D = json.load(open(sys.argv[1])), sys.argv[3]
a = lambda n: "0x" + "%040x" % (0xa11ce000 + n)
names = ["registry", "rollup", "inbox", "outbox", "feeJuice", "feeJuicePortal", "coinIssuer", "rewardDistributor", "governanceProposer",
         "governance", "stakingAsset", "feeAssetHandler", "stakingAssetHandler", "zkPassportVerifier", "gse", "dateGatedRelayer"]
addresses = {n: a(i + 1) for i, n in enumerate(names)} | {"multicall3": "0xca11bde05977b3631167028862be2a173976ca11"}
cfg = d["cfg"] | {"manaLimit": str(d["cfg"]["manaLimit"])}
json.dump(dict(port=8545, jip2="ws://127.0.0.1:19800", chainSpec=d["chainSpec"], service=d["service"], jamboree=sys.argv[4],
               cfg=cfg, addresses=addresses, work=f"{D}/relay", log=f"{D}/jam-l1.log"), open(sys.argv[2], "w"), indent=1)
PY
node "$HERE/jam-l1.mjs" "$D/jam-l1.json" > "$D/jam-l1.out" 2>&1 & pids+=($!)
sleep 2
# test mode, as `aztec start --local-network` runs: transactions' client proofs aren't verified
# the sequencer's keys: the first account of Foundry's and Aztec's test mnemonic, as the local network uses
KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcaee95b6caee5d1b5
SEQ_PUBLISHER_PRIVATE_KEYS=$KEY VALIDATOR_PRIVATE_KEYS=$KEY COINBASE=0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 \
SEQ_BUILD_CHECKPOINT_IF_EMPTY=${SEQ_BUILD_CHECKPOINT_IF_EMPTY:-true} \
PROVER_REAL_PROOFS=false \
ETHEREUM_HOSTS=http://127.0.0.1:8545 L1_CHAIN_ID=31337 REGISTRY_CONTRACT_ADDRESS=$REGISTRY TEST_ACCOUNTS=true SPONSORED_FPC=true P2P_ENABLED=false \
DATA_DIRECTORY=$D/node-data BLOB_FILE_STORE_URLS=file://$D/blobs BLOB_FILE_STORE_UPLOAD_URL=file://$D/blobs \
ARCHIVER_POLLING_INTERVAL_MS=1000 ARCHIVER_VIEM_POLLING_INTERVAL_MS=1000 LOG_LEVEL=${LOG_LEVEL:-info} \
  "$HERE/node_modules/.bin/aztec" start ${@:---node} --port "${NODE_PORT:-8080}" > "$D/node.log" 2>&1 & pids+=($!)
echo "Aztec on JAM: devnet, jam-l1 on :8545, Aztec's node on :${NODE_PORT:-8080} (logs in $D)"
wait
