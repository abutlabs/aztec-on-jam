#!/usr/bin/env python3
"""Talk to a JAM service over JIP-2 with jamswap's offchain modules (stdlib only).
Runs inside a container on the net (nets/lasair6.sh wraps it).

  jamctl.py submit <service_id> <payload-hex>     one work-item in its own package; waits
                                                   until the package is Ready or Failed
  jamctl.py read   <service_id> <key-hex|"text">  a storage value at the best block (hex)
  jamctl.py head                                   best and finalized slot
"""
import os, sys, time
sys.path.insert(0, os.environ.get("OFFCHAIN", "/offchain"))
import chain  # noqa: E402

RPC = os.environ.get("CHAIN_RPC", "ws://reader:19800")
SPEC = os.environ.get("CHAIN_SPEC", "/shared/spec.json")


def key_bytes(s):
    return bytes.fromhex(s) if all(c in "0123456789abcdefABCDEF" for c in s) and len(s) % 2 == 0 and len(s) > 8 else s.encode()


def main(argv):
    cmd = argv[1]
    if cmd == "head":
        ch = chain.Jip2Chain(None, RPC, chain_spec=SPEC)
        print("best", ch.head().slot, "finalized", ch.finalized().slot)
        return 0
    sid = int(argv[2])
    ch = chain.Jip2Chain(sid, RPC, chain_spec=SPEC)
    if cmd == "read":
        v = ch.read(key_bytes(argv[3]))
        print(v.hex() if v else "(none)")
        return 0
    if cmd == "submit":
        payload = bytes.fromhex(argv[3])
        r = ch.submit(payload)
        print("submitted", r["package_hash"], "core", r["core"], "anchor slot", r["anchor_slot"])
        deadline = time.time() + float(os.environ.get("WAIT", "240"))
        last = None
        while time.time() < deadline:
            st = ch.package_status(r)
            if st != last:
                print("status", st, "at slot", ch.head().slot); last = st
            if isinstance(st, dict) and ("Ready" in st or "Failed" in st):
                return 0 if "Ready" in st else 2
            time.sleep(3)
        print("timeout"); return 3
    print(__doc__); return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
