#!/usr/bin/env bash
# Aztec v6's released software on a JAM testnet: the settlement service on six JAM validators
# (services/aztec-settlement/devnet.sh), our JAM sidecar (jam-l1.mjs) as the node's "Ethereum",
# Aztec's sequencer node, Aztec's prover (broker, prover node and agent), and the wallet.
#
#   tools/aztec-local/on-jam.sh            start everything; Ctrl-C stops everything
#   tools/aztec-local/on-jam.sh down       stop a run left behind (any terminal)
#
#   DEVNET_NET=jb-lasair6   six lasair validators with the JIP's host calls (needs ../lasair built);
#                           the default, polkajam, is PolkaJam's own testnet
#   PROVER=0                skip Aztec's prover (default 1)
#   PROVER_AGENT_COUNT      prover agents (default 1)
#   PROVER_REAL_PROOFS=false  Aztec's test mode: client proofs not verified (default true)
#   NODE_PORT               the node's port (default 8080, where Aztec's wallets look)
# Logs: out/aztec-devnet/ — devnet.log, jam-l1.log, node.log, broker.log, prover.log, wallet.log.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$HERE/../.." && pwd); D=$ROOT/out/aztec-devnet
NODE_PORT=${NODE_PORT:-8080}; PROVER=${PROVER:-1}; WALLET_PORT=${WALLET_PORT:-19720}
PORTS="8545 $NODE_PORT 8083 8084 8885 $WALLET_PORT 19800 42950"
RUN_PID=$ROOT/out/aztec-on-jam.pid     # the running on-jam.sh; outside $D, which each run wipes

# --- down: stop a run from another terminal ---------------------------------------------------
# The run's own trap stops exactly what it started; anything left after that (a run killed
# with -9) is cleared by name.
if [ "${1:-}" = down ]; then
  if [ -f "$RUN_PID" ] && kill -0 "$(cat "$RUN_PID")" 2>/dev/null; then
    kill "$(cat "$RUN_PID")"
    for _ in $(seq 1 30); do kill -0 "$(cat "$RUN_PID" 2>/dev/null)" 2>/dev/null || break; sleep 1; done
  fi
  pkill -f "$HERE/node_modules/@aztec-labs/aztec/" 2>/dev/null || true
  pkill -f "$HERE/jam-l1.mjs" 2>/dev/null || true
  pkill -f "$ROOT/services/aztec-settlement/devnet.sh" 2>/dev/null || true
  pkill -f "$ROOT/wallet/serve.mjs" 2>/dev/null || true
  NET=jb-lasair6 "$ROOT/nets/jb.sh" down >/dev/null 2>&1 || true
  rm -f "$RUN_PID"
  echo "Aztec on JAM: stopped"; exit 0
fi

# --- Node 22+ (Aztec's packages and the sidecar need it) ---------------------------------------
if [ "$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)" -lt 22 ]; then
  for v in $(ls -d "$HOME"/.nvm/versions/node/v* 2>/dev/null | sort -V); do PATH="$v/bin:$PATH"; done
  export PATH
  [ "$(node -p 'process.versions.node.split(".")[0]')" -ge 22 ] || { echo "on-jam.sh: Node 22+ is needed" >&2; exit 2; }
fi

T0=$(date +%s)
say() { printf '[%4ss] %s\n' "$(( $(date +%s) - T0 ))" "$*"; }
fail() { echo; echo "on-jam.sh: $1" >&2; [ -n "${2:-}" ] && [ -f "$2" ] && { echo "--- last lines of $2:" >&2; grep -E "rror|ERROR|refus|in use" "$2" | tail -5 >&2 || tail -8 "$2" >&2; }; exit 1; }

# --- preflight: nothing may already hold our ports ---------------------------------------------
busy=""
for p in $PORTS; do
  who=$(lsof -nP -iTCP:"$p" -sTCP:LISTEN 2>/dev/null | awk 'NR==2{print $1" (pid "$2")"}' || true)
  if [ -n "$who" ]; then busy="$busy\n  port $p: $who"; fi
done
if [ -f "$RUN_PID" ] && kill -0 "$(cat "$RUN_PID")" 2>/dev/null; then busy="$busy\n  another on-jam.sh is running (pid $(cat "$RUN_PID"))"; fi
if pgrep -f "lasair_client|polkajam-testnet" >/dev/null 2>&1; then busy="$busy\n  a JAM testnet is already running (nets/demo.sh or another on-jam.sh?)"; fi
if [ -n "$busy" ]; then
  printf 'on-jam.sh: something is already running:%b\n\nStop it first:\n  tools/aztec-local/on-jam.sh down     (a previous on-jam.sh)\n  nets/demo.sh down                    (the first demo)\n' "$busy" >&2
  exit 1
fi

# Aztec v6's genesis: the test accounts and the SponsoredFPC funded (TEST_ACCOUNTS=true,
# SPONSORED_FPC=true; computed by the node's own getGenesisValues), and its circuits
GENESIS_ARCHIVE=0x0ad1dcc0e08d2b1c82a79af5685598e631e881187c3c9734ad28df5dbcf5db48
VK_TREE_ROOT=0x22fff5de6ce590153df4468f7f0b152188a803d91b3c9c648b91d93d5913f219
PROTOCOL_CONTRACTS_HASH=0x0f54271c52865841a77aaa66036eed901aff22fdce9f08e123ec8b205c9854d7
REGISTRY=0x00000000000000000000000000000000a11ce001
rm -rf "$D"; mkdir -p "$D/blobs" "$D/node-data" "$D/bb-work"
pids=()
track() { pids+=("$1"); }
echo $$ > "$RUN_PID"
# stop exactly what this run started, children first, then this run's testnet
stop_own() {
  local rc=$?
  trap - EXIT INT TERM
  echo; echo "Aztec on JAM: stopping"
  for p in "${pids[@]}"; do pkill -P "$p" 2>/dev/null || true; kill "$p" 2>/dev/null || true; done
  if [ "${DEVNET_NET:-polkajam}" != polkajam ]; then NET=$DEVNET_NET "$ROOT/nets/jb.sh" down >/dev/null 2>&1 || true; fi
  wait 2>/dev/null || true
  if [ "$(cat "$RUN_PID" 2>/dev/null)" = "$$" ]; then rm -f "$RUN_PID"; fi
  exit "$rc"
}
trap stop_own EXIT INT TERM
if [ "$PROVER" = 1 ]; then export PROOF_SUBMISSION_EPOCHS=${PROOF_SUBMISSION_EPOCHS:-10000}; fi

# --- 1. the JAM testnet and the settlement service ---------------------------------------------
say "1/5 starting six ${DEVNET_NET:-polkajam} validators and deploying the settlement service (about 2–3 minutes)"
"$ROOT/services/aztec-settlement/devnet.sh" "$D" "$GENESIS_ARCHIVE" "$VK_TREE_ROOT" "$PROTOCOL_CONTRACTS_HASH" > "$D/devnet.log" 2>&1 & track $!
until grep -qE "devnet up|rror|never finalized|in use" "$D/devnet.log" 2>/dev/null; do
  kill -0 "${pids[0]}" 2>/dev/null || fail "the JAM testnet stopped while starting" "$D/devnet.log"; sleep 3
done
grep -q "devnet up" "$D/devnet.log" || fail "the JAM testnet did not start" "$D/devnet.log"
say "    JAM testnet up; settlement service $(python3 -c "import json; print(json.load(open('$D/devnet.json'))['service'])") deployed"

# --- 2. the JAM sidecar -------------------------------------------------------------------------
python3 - "$D/devnet.json" "$D/jam-l1.json" "$D" "$ROOT/../jamboree/target/release/jamboree" <<'PY'
import json, sys
d, D = json.load(open(sys.argv[1])), sys.argv[3]
a = lambda n: "0x" + "%040x" % (0xa11ce000 + n)
names = ["registry", "rollup", "inbox", "outbox", "feeJuice", "feeJuicePortal", "coinIssuer", "rewardDistributor", "governanceProposer",
         "governance", "stakingAsset", "feeAssetHandler", "stakingAssetHandler", "zkPassportVerifier", "gse", "dateGatedRelayer"]
addresses = {n: a(i + 1) for i, n in enumerate(names)} | {"multicall3": "0xca11bde05977b3631167028862be2a173976ca11"}
cfg = d["cfg"] | {"manaLimit": str(d["cfg"]["manaLimit"])}
json.dump(dict(port=8545, jip2=d.get("jip2", "ws://127.0.0.1:19800"), chainSpec=d["chainSpec"], service=d["service"], jamboree=sys.argv[4],
               rootRollupVk=f"{D}/root-rollup.vk",
               cfg=cfg, addresses=addresses, work=f"{D}/relay", log=f"{D}/jam-l1.log"), open(sys.argv[2], "w"), indent=1)
PY
node "$HERE/jam-l1.mjs" "$D/jam-l1.json" > "$D/jam-l1.out" 2>&1 & track $!
for _ in $(seq 1 20); do grep -q "jam-l1 on" "$D/jam-l1.out" 2>/dev/null && break; sleep 1; done
grep -q "jam-l1 on" "$D/jam-l1.out" || fail "the JAM sidecar did not start" "$D/jam-l1.out"
say "2/5 JAM sidecar up on :8545 (Aztec's node will treat it as Ethereum)"

# --- 3. Aztec's sequencer node -----------------------------------------------------------------
# Real proofs by default: the node verifies each transaction's client proof before pooling it.
# The sequencer's keys: the first account of Foundry's and Aztec's test mnemonic, as the local network uses.
KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcaee95b6caee5d1b5
say "3/5 starting Aztec's sequencer node"
SEQ_PUBLISHER_PRIVATE_KEYS=$KEY VALIDATOR_PRIVATE_KEYS=$KEY COINBASE=0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 \
SEQ_BUILD_CHECKPOINT_IF_EMPTY=${SEQ_BUILD_CHECKPOINT_IF_EMPTY:-true} \
PROVER_REAL_PROOFS=${PROVER_REAL_PROOFS:-true} BB_WORKING_DIRECTORY=$D/bb-work \
ETHEREUM_HOSTS=http://127.0.0.1:8545 L1_CHAIN_ID=31337 REGISTRY_CONTRACT_ADDRESS=$REGISTRY TEST_ACCOUNTS=true SPONSORED_FPC=true P2P_ENABLED=false \
DATA_DIRECTORY=$D/node-data BLOB_FILE_STORE_URLS=file://$D/blobs BLOB_FILE_STORE_UPLOAD_URL=file://$D/blobs \
ARCHIVER_POLLING_INTERVAL_MS=1000 ARCHIVER_VIEM_POLLING_INTERVAL_MS=1000 LOG_LEVEL=${LOG_LEVEL:-info} \
  "$HERE/node_modules/.bin/aztec" start --node --sequencer --port "$NODE_PORT" > "$D/node.log" 2>&1 & NODE_PID=$!; track $NODE_PID
until grep -q "Aztec Node started" "$D/node.log" 2>/dev/null; do
  kill -0 "$NODE_PID" 2>/dev/null || fail "Aztec's node stopped while starting" "$D/node.log"; sleep 2
done
say "    Aztec node up on :$NODE_PORT"

# --- 4. Aztec's prover: broker, then prover node with its agent (Aztec's operator-docs layout) ---
# Where Aztec's docs say `--network mainnet`, this devnet gives every role the registry address and
# the genesis flags (each checks the genesis and stands by on a mismatch); blobs come from the
# sequencer's file store; the prover reads transactions from our node (P2P is off). The broker
# keeps work for as many epochs as the proof window allows: its default (1) discards a slow
# prover's jobs before they finish.
if [ "$PROVER" = 1 ]; then
  say "4/5 starting Aztec's prover: broker, prover node, ${PROVER_AGENT_COUNT:-1} agent(s)"
  mkdir -p "$D/prover-data" "$D/bb-prover" "$D/noir-work" "$D/broker-data"
  ETHEREUM_HOSTS=http://127.0.0.1:8545 L1_CHAIN_ID=31337 REGISTRY_CONTRACT_ADDRESS=$REGISTRY \
  TEST_ACCOUNTS=true SPONSORED_FPC=true RPC_MAX_BODY_SIZE=${RPC_MAX_BODY_SIZE:-64mb} \
  PROVER_BROKER_MAX_EPOCHS_TO_KEEP_RESULTS_FOR=${PROOF_SUBMISSION_EPOCHS:-10000} \
  DATA_DIRECTORY=$D/broker-data LOG_LEVEL=${LOG_LEVEL:-info} \
    "$HERE/node_modules/.bin/aztec" start --prover-broker --port 8084 > "$D/broker.log" 2>&1 & BROKER_PID=$!; track $BROKER_PID
  for _ in $(seq 1 60); do curl -s -o /dev/null http://127.0.0.1:8084 && break; kill -0 "$BROKER_PID" 2>/dev/null || fail "Aztec's prover broker stopped" "$D/broker.log"; sleep 1; done
  PROVER_PUBLISHER_PRIVATE_KEY=0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d \
  PROVER_AGENT_COUNT=${PROVER_AGENT_COUNT:-1} PROVER_BROKER_HOST=http://127.0.0.1:8084 \
  PROVER_REAL_PROOFS=${PROVER_REAL_PROOFS:-true} BB_WORKING_DIRECTORY=$D/bb-prover \
  BB_BINARY_PATH=$HERE/node_modules/@aztec-foundation/bb.js/build/arm64-macos/bb \
  NOIR_EXECUTE_BINARY_PATH=$HOME/.nargo/bin/noir-execute NOIR_EXECUTE_WORKING_DIRECTORY=$D/noir-work \
  ETHEREUM_HOSTS=http://127.0.0.1:8545 L1_CHAIN_ID=31337 REGISTRY_CONTRACT_ADDRESS=$REGISTRY P2P_ENABLED=false \
  TEST_ACCOUNTS=true SPONSORED_FPC=true TX_COLLECTION_NODE_RPC_URLS=http://127.0.0.1:$NODE_PORT \
  RPC_MAX_BODY_SIZE=${RPC_MAX_BODY_SIZE:-64mb} \
  DATA_DIRECTORY=$D/prover-data BLOB_FILE_STORE_URLS=file://$D/blobs BLOB_FILE_STORE_UPLOAD_URL=file://$D/blobs \
  ARCHIVER_POLLING_INTERVAL_MS=1000 ARCHIVER_VIEM_POLLING_INTERVAL_MS=1000 LOG_LEVEL=${LOG_LEVEL:-info} \
    "$HERE/node_modules/.bin/aztec" start --prover-node --prover-agent --port 8083 --admin-port 8885 > "$D/prover.log" 2>&1 & PROVER_PID=$!; track $PROVER_PID
  until grep -q "Started Prover Node" "$D/prover.log" 2>/dev/null; do
    kill -0 "$PROVER_PID" 2>/dev/null || fail "Aztec's prover stopped while starting" "$D/prover.log"; sleep 2
  done
  say "    prover up (broker :8084, prover node :8083)"
else
  say "4/5 prover skipped (PROVER=0)"
fi

# --- ready means usable: a transaction built before block 1 is valid only in block 1 ----------
say "    waiting for the chain's first block (about a minute)"
until [ "$(curl -s -m3 -X POST -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"node_getBlockNumber","params":[]}' http://127.0.0.1:$NODE_PORT 2>/dev/null | grep -oE '"result":[0-9]+' | cut -d: -f2)" -ge 1 ] 2>/dev/null; do
  kill -0 "$NODE_PID" 2>/dev/null || fail "Aztec's node stopped" "$D/node.log"; sleep 3
done
say "    block 1 is in"

# --- 5. the wallet ------------------------------------------------------------------------------
if [ -f "$ROOT/wallet/dist/index.html" ]; then
  node "$ROOT/wallet/serve.mjs" "$WALLET_PORT" > "$D/wallet.log" 2>&1 & track $!
  sleep 1
  say "5/5 wallet up"
  WALLET_LINE="  wallet:   http://localhost:$WALLET_PORT/?node=http://localhost:$NODE_PORT"
else
  say "5/5 wallet not built — build it once:  cd wallet && npm install && npm run build"
  WALLET_LINE="  wallet:   build it (cd wallet && npm install && npm run build), then rerun"
fi

cat <<EOF

Aztec on JAM is running — $(( $(date +%s) - T0 ))s to start.
$WALLET_LINE
  node:     http://localhost:$NODE_PORT  (chain 31337)
  try it:   node tools/aztec-local/send-tx.mjs     (a private transfer, end to end)
  logs:     out/aztec-devnet/
  stop:     Ctrl-C here, or  tools/aztec-local/on-jam.sh down
EOF

# stay up; report if a part dies
while true; do
  for p in "${pids[@]}"; do kill -0 "$p" 2>/dev/null || fail "a process stopped (pid $p); see out/aztec-devnet/"; done
  sleep 5
done
