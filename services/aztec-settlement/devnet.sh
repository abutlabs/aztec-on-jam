#!/usr/bin/env bash
# #53, #66: a JAM testnet with the settlement service configured for Aztec v6's own genesis,
# kept up until stopped (Ctrl-C or kill), for Aztec's node to settle on through
# tools/aztec-local/jam-l1.mjs. The net is DEVNET_NET: polkajam (the default; spikes/lib's
# PolkaJam v0.8.0 testnet, JIP-2 on 19800) or jb-lasair6 (nets/jb.sh: six lasair validators,
# JIP-2 on 42950, the proposed curve calls on from slot 0 unless LASAIR_JIP8_FROM_SLOT says
# otherwise — the service falls back to PVM code without them).
#   devnet.sh <out dir> <genesis archive> <vk tree root> <protocol contracts hash>
# Writes <out dir>/devnet.json: the service id, the chain spec, the JIP-2 URL, Aztec's genesis
# time and the cfg.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd); O=$(mkdir -p "$1" && cd "$1" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
DEVNET_NET=${DEVNET_NET:-polkajam}
if [ "$DEVNET_NET" = polkajam ]; then
  . "$HERE/../../spikes/lib/polkajam.sh"
  RPC_URL=ws://127.0.0.1:19800
else
  SUB=$(cd "$ROOT/.." && pwd)
  JB=${JAMBOREE:-$SUB/jamboree/target/release/jamboree}
  export NET=$DEVNET_NET LASAIR_JIP8_FROM_SLOT=${LASAIR_JIP8_FROM_SLOT:-0} LASAIR_JIP8_105_FROM_SLOT=${LASAIR_JIP8_105_FROM_SLOT:-0}
  RPC_URL=ws://127.0.0.1:42950
  trap '"$ROOT/nets/jb.sh" down || true' EXIT INT TERM
fi
BUILD=$SUB/jamswap/tools/jam080/builder/target/release/jam080-build

# The net first: the cfg's JAM era (Unix time of JAM timeslot 0) depends on it. Both run their
# timeslots on the wall clock: PolkaJam's dev chain from JAM's common era, an all-lasair jb net
# from its own start, which jb.sh writes to its era file. One timebase for the service's clock
# and the facade's "L1 blocks".
if [ "$DEVNET_NET" = polkajam ]; then
  pj_start "$O"
  JAM_ERA=1735732800
else
  "$ROOT/nets/jb.sh" up
  cp "$ROOT/out/$DEVNET_NET/spec.json" "$O/spec.json"
  JAM_ERA=$(cat "$ROOT/out/$DEVNET_NET/era")
fi
GENESIS_TIME=$(date +%s)
# PROOF_SUBMISSION_EPOCHS above about 14,600 (24.8 days) overflows Node's setTimeout in the prover
# The epoch verifier's key: Aztec's real RootRollup vk, whose sha256 goes into cfg (#69). The
# facade attaches the key bytes to each epoch-proof work item; accumulate checks the hash.
VK_FILE=$O/root-rollup.vk
(cd "$ROOT/tools/aztec-local" && node --input-type=module -e "
import { ServerCircuitVks } from '@aztec-labs/noir-protocol-circuits-types/server/vks';
import { writeFileSync } from 'fs';
writeFileSync(process.argv[1], ServerCircuitVks['RootRollupArtifact'].keyAsBytes);
" "$VK_FILE") 2>/dev/null || VK_FILE=""
# Aztec's sha256ToField, as the service computes it (crates/aztec-l1): 0x00 ‖ the first 31 bytes
VK_HASH=$([ -n "$VK_FILE" ] && echo "00$(shasum -a 256 "$VK_FILE" | cut -c1-62)" || echo "")
# COMMITTEE_ADDRESSES: the validators, comma-separated (on-jam.sh's COMMITTEE=n); none means an
# open committee, as Aztec's local network runs. SAMPLE_SEED fixes every epoch's proposer order.
python3 - "$HERE/cfg.bin" "$O/devnet-cfg.json" "$2" "$3" "$4" "$GENESIS_TIME" "$JAM_ERA" "${PROOF_SUBMISSION_EPOCHS:-1}" "${EPOCH_SLOTS:-4}" "$VK_HASH" "${COMMITTEE_ADDRESSES:-}" "${SAMPLE_SEED:-0x2a}" <<'PY'
import json, struct, sys
out, js, archive, vk_tree, protocol, genesis, era, window, eslots, vk_hash, committee, seed = sys.argv[1:13]
committee = [a.strip().lower() for a in committee.split(",") if a.strip()]
assert len(committee) <= 16, "the service holds at most 16 committee members"
cfg = dict(genesisArchive=archive, genesisTime=int(genesis), slotSeconds=36, epochSlots=int(eslots), minFee=1000, manaLimit=2**63,
           chainId=31337, version=1, vkTreeRoot=vk_tree, protocolContractsHash=protocol,
           proofSubmissionEpochs=int(window),   # PROOF_SUBMISSION_EPOCHS: a slow prover's window (#69)
           jamEra=int(era), epochVkHash="0x" + (vk_hash or "00" * 32),
           committee=committee, sampleSeed="0x" + seed[2:].rjust(64, "0"))
w = lambda h: bytes.fromhex(h[2:].rjust(64, "0"))
open(out, "wb").write(w(archive) + struct.pack(">QQQ", cfg["genesisTime"], 36, cfg["epochSlots"]) + (1000).to_bytes(16, "big")
                      + (2**63).to_bytes(32, "big") + (31337).to_bytes(32, "big") + (1).to_bytes(32, "big")
                      + w(cfg["epochVkHash"]) + w(vk_tree) + w(protocol) + struct.pack(">Q", cfg["proofSubmissionEpochs"])
                      + bytes.fromhex("00000000000000000000000000000000a11ce002")   # the facade's Rollup
                      + w(cfg["sampleSeed"]) + bytes([len(committee)])                  # the committee (#54, #68)
                      + b"".join(bytes.fromhex(a[2:]) for a in committee).ljust(320, b"\0")
                      + struct.pack(">Q", cfg["jamEra"]))
json.dump(cfg, open(js, "w"))
PY
BLOB=$("$BUILD" "$HERE" --out "$O" | awk '{print $1}')
if [ "$DEVNET_NET" = polkajam ]; then
  pj_deploy "$BLOB"
  cp "$PJ_W/spec.json" "$O/spec.json"
  SID=$PJ_SID
else
  SID=$("$JB" rpc --url "$RPC_URL" --chain-spec "$O/spec.json" deploy "$BLOB" --fresh | sed -n 's/^SERVICE_ID=//p')
  [ -n "$SID" ] || { echo "devnet: deploy on $DEVNET_NET gave no service id" >&2; exit 1; }
fi
python3 -c "import json, sys; c = json.load(open(sys.argv[1])); json.dump({'service': int(sys.argv[2]), 'chainSpec': sys.argv[3], 'jip2': sys.argv[5], 'cfg': c}, open(sys.argv[4], 'w'))" "$O/devnet-cfg.json" "$SID" "$O/spec.json" "$O/devnet.json" "$RPC_URL"
echo "devnet up: settlement service $SID on $DEVNET_NET, Aztec genesis time $GENESIS_TIME; $O/devnet.json"
while true; do sleep 3600; done
