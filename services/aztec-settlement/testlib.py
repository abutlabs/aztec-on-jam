#!/usr/bin/env python3
"""Shared by measure.sh (#49) and inbox.sh (#50): the service's cfg and its work-item payloads.

  testlib.py cfg <gen dir> <genesis unix time> <out>       cfg.bin: genesis archive, time, 6 s slots,
                                                           32-slot epochs, fee 1000, no mana limit,
                                                           chain id 31337, version 1
  testlib.py payload <tag> <gen dir> <name> <bucket hint> [alter] > p.bin
      alter: blobsHash (flip its last bit), timestamp (+6 s), inbox=0x.. (replace the hash)
  testlib.py extrinsics <gen dir> <name>                    jamboree args for its blobs and exports
"""
import json, struct, sys

def cfg(gen, genesis, out):
    archive = bytes.fromhex(json.load(open(f"{gen}/genesis.json"))["archive"][2:])
    open(out, "wb").write(archive + struct.pack(">QQQ", int(genesis), 6, 32) + (1000).to_bytes(16, "big")
                          + b"\xff" * 32 + (31337).to_bytes(32, "big") + (1).to_bytes(32, "big"))

def payload(tag, gen, name, hint, alter=None):
    h = bytearray(open(f"{gen}/{name}.header", "rb").read())
    if alter == "blobsHash":
        h[64 + 31] ^= 1
    elif alter == "timestamp":
        h[192:200] = (int.from_bytes(h[192:200], "big") + 6).to_bytes(8, "big")
    elif alter and alter.startswith("inbox="):
        h[96:128] = bytes.fromhex(alter[6:].removeprefix("0x").rjust(64, "0"))
    blobs = open(f"{gen}/{name}.blobs", "rb").read()
    archive = open(f"{gen}/{name}.archive", "rb").read()
    sys.stdout.buffer.write(bytes([1, int(tag)]) + bytes(h) + archive + int(hint).to_bytes(8, "little") + blobs)

def extrinsics(gen, name):
    n = json.load(open(f"{gen}/{name}.json"))["blobs"]
    print(" ".join(f"--extrinsic @{gen}/{name}.blob{j}" for j in range(n)), f"--exports {32 * n}")

if __name__ == "__main__":
    {"cfg": cfg, "payload": payload, "extrinsics": extrinsics}[sys.argv[1]](*sys.argv[2:])
