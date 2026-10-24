#!/usr/bin/env python3
"""Shared by measure.sh (#49) and inbox.sh (#50): the service's cfg and its work-item payloads.

  testlib.py cfg <gen dir> <genesis unix time> <out> [vk|-] [committee.json]  cfg.bin: genesis archive, time, 6 s slots,
                                                           32-slot epochs, fee 1000, no mana limit,
                                                           chain id 31337, version 1, the vk's hash,
                                                           vk tree root 0x01, protocol contracts hash
                                                           0x02, 1 proof submission epoch
  testlib.py epoch <tag> <start> <end> <vk> <pubs.json> <proof> <batched> [alter] > p.bin
      alter: pub=<i>:0x.. (replace a public input), z (flip the opening's z)
  testlib.py payload <tag> <gen dir> <name> <bucket hint> [alter] > p.bin
      alter: blobsHash (flip its last bit), timestamp (+6 s), inbox=0x.. (replace the hash)
  testlib.py extrinsics <gen dir> <name>                    jamboree args for its blobs and exports
"""
import json, struct, sys

def field_hash(b):
    import hashlib
    return b"\0" + hashlib.sha256(b).digest()[:31]

ROLLUP = bytes.fromhex("00000000000000000000000000000000a11ce002")   # the facade's Rollup address

def cfg(gen, genesis, out, vk=None, committee=None):
    """committee: a JSON file {addresses, seed}, as gen.mjs's spec gives it; none by default."""
    archive = bytes.fromhex(json.load(open(f"{gen}/genesis.json"))["archive"][2:])
    vk_hash = field_hash(open(vk, "rb").read()) if vk and vk != "-" else bytes(32)
    members, seed = [], 0
    if committee:
        c = json.load(open(committee)); members, seed = [bytes.fromhex(a[2:]) for a in c["addresses"]], int(c["seed"])
    open(out, "wb").write(archive + struct.pack(">QQQ", int(genesis), 6, 32) + (1000).to_bytes(16, "big")
                          + b"\xff" * 32 + (31337).to_bytes(32, "big") + (1).to_bytes(32, "big")
                          + vk_hash + (1).to_bytes(32, "big") + (2).to_bytes(32, "big") + struct.pack(">Q", 1)
                          + ROLLUP + seed.to_bytes(32, "big") + bytes([len(members)]) + b"".join(members).ljust(320, b"\0"))

# no attestations: modifier 0, empty indices and signatures, no signers, a zero proposer signature
EMPTY_ATTESTATIONS = bytes(32) + bytes(2) + bytes(2) + bytes(2) + bytes(65)

def epoch(tag, start, end, vk, pubs, proof, batched, alter=None):
    vk, proof, batched = open(vk, "rb").read(), open(proof, "rb").read(), bytearray(open(batched, "rb").read())
    pis = [int(x, 16).to_bytes(32, "big") for x in json.load(open(pubs))]
    if alter and alter.startswith("pub="):
        i, v = alter[4:].split(":")
        pis[int(i)] = int(v, 16).to_bytes(32, "big")
    elif alter == "z":
        batched[63] ^= 1
    sys.stdout.buffer.write(bytes([2, int(tag)]) + int(start).to_bytes(8, "little") + int(end).to_bytes(8, "little")
                            + len(vk).to_bytes(2, "little") + vk + b"".join(pis) + len(proof).to_bytes(4, "little") + proof + bytes(batched))

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
    try:
        att = open(f"{gen}/{name}.attest", "rb").read()
    except FileNotFoundError:
        att = EMPTY_ATTESTATIONS
    sys.stdout.buffer.write(bytes([1, int(tag)]) + bytes(h) + archive + int(hint).to_bytes(8, "little") + blobs + att)

def extrinsics(gen, name):
    n = json.load(open(f"{gen}/{name}.json"))["blobs"]
    print(" ".join(f"--extrinsic @{gen}/{name}.blob{j}" for j in range(n)), f"--exports {32 * n}")

if __name__ == "__main__":
    {"cfg": cfg, "payload": payload, "extrinsics": extrinsics, "epoch": epoch}[sys.argv[1]](*sys.argv[2:])
