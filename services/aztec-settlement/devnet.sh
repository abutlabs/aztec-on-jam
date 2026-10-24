#!/usr/bin/env bash
# #53: a PolkaJam testnet (spikes/lib/polkajam.sh, RPC 19800) with the settlement service
# configured for Aztec v6's own genesis, kept up until stopped (Ctrl-C or kill), for Aztec's
# node to settle on through tools/aztec-local/jam-l1.mjs.
#   devnet.sh <out dir> <genesis archive> <vk tree root> <protocol contracts hash>
# Writes <out dir>/devnet.json: the service id, the chain spec, Aztec's genesis time and the cfg.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$(mkdir -p "$1" && cd "$1" && pwd)
. "$HERE/../../spikes/lib/polkajam.sh"
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build
GENESIS_TIME=$(date +%s)
python3 - "$HERE/cfg.bin" "$O/devnet-cfg.json" "$2" "$3" "$4" "$GENESIS_TIME" <<'PY'
import json, struct, sys
out, js, archive, vk_tree, protocol, genesis = sys.argv[1:7]
cfg = dict(genesisArchive=archive, genesisTime=int(genesis), slotSeconds=36, epochSlots=4, minFee=1000, manaLimit=2**63,
           chainId=31337, version=1, vkTreeRoot=vk_tree, protocolContractsHash=protocol, proofSubmissionEpochs=1)
w = lambda h: bytes.fromhex(h[2:].rjust(64, "0"))
open(out, "wb").write(w(archive) + struct.pack(">QQQ", cfg["genesisTime"], 36, 4) + (1000).to_bytes(16, "big")
                      + (2**63).to_bytes(32, "big") + (31337).to_bytes(32, "big") + (1).to_bytes(32, "big")
                      + bytes(32) + w(vk_tree) + w(protocol) + struct.pack(">Q", 1)
                      + bytes.fromhex("00000000000000000000000000000000a11ce002")   # the facade's Rollup
                      + bytes(32) + bytes([0]) + bytes(320))                            # no committee, as the local network
json.dump(cfg, open(js, "w"))
PY
BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
pj_start "$O"
pj_deploy "$BLOB"
cp "$PJ_W/spec.json" "$O/spec.json"
python3 -c "import json, sys; c = json.load(open(sys.argv[1])); json.dump({'service': int(sys.argv[2]), 'chainSpec': sys.argv[3], 'cfg': c}, open(sys.argv[4], 'w'))" "$O/devnet-cfg.json" "$PJ_SID" "$O/spec.json" "$O/devnet.json"
echo "devnet up: settlement service $PJ_SID, Aztec genesis time $GENESIS_TIME; $O/devnet.json"
while true; do sleep 3600; done
