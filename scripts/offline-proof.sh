#!/usr/bin/env bash
# A real transfer, proved and verified with no chain: the client claims a deposit and pays 40,
# then bb checks the proof against the evm verification key the service embeds, and a changed
# public input must fail. CI runs this; so can anyone with the toolchain.
set -euo pipefail
HERE=$(cd "$(dirname "$0")/.." && pwd); T=${OUT:-$HERE/out/offline}; rm -rf "$T"; mkdir -p "$T"
export CLIENT_STATE=$T/state.json
BB=${BB:-$HOME/.bb/versions/bb-5.0.0}; [ -x "$BB" ] || BB=bb
"$HERE/client/privacy.py" deposit A 100 >/dev/null
"$HERE/client/privacy.py" transfer A deposit:0 - B 40 0 0 0 "$T/tx.hex" | head -1
python3 - "$T" <<'PY'
import sys
T = sys.argv[1]
p = bytes.fromhex(open(f"{T}/tx.hex").read())
n = int.from_bytes(p[:2], "little"); m = int.from_bytes(p[2 + n:4 + n], "little")
open(f"{T}/public_inputs", "wb").write(p[2:2 + n]); open(f"{T}/proof", "wb").write(p[4 + n:4 + n + m])
bad = bytearray(p[2:2 + n]); bad[3 * 32 + 31] ^= 1; open(f"{T}/public_inputs.bad", "wb").write(bad)
print(f"{n // 32} public inputs, a {m}-byte proof, {len(p) - 4 - n - m} bytes of sealed notes")
PY
VK=$HERE/services/privacy/fixtures/transfer.vk
"$BB" verify -t evm -k "$VK" -p "$T/proof" -i "$T/public_inputs" && echo "bb verify: the proof holds against the service's key"
if "$BB" verify -t evm -k "$VK" -p "$T/proof" -i "$T/public_inputs.bad" >/dev/null 2>&1; then echo "FAIL: a changed public input verified"; exit 1; fi
echo "bb verify: a changed public input is refused"
