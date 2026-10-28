# The shim's input from a proof, a vk and bb's CRS files (aztec-on-jam#62).
import sys, struct, os
proof, vk, crs, n_bn, n_gr, out = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5]), sys.argv[6]
p = open(proof, "rb").read(); v = open(vk, "rb").read()
with open(os.path.join(crs, "bn254_g1.dat"), "rb") as f: bn = f.read(64 * n_bn)
g2 = open(os.path.join(crs, "bn254_g2.dat"), "rb").read()
with open(os.path.join(crs, "grumpkin_g1_v2.flat.dat"), "rb") as f: gr = f.read(64 * n_gr)
body = struct.pack("<I", len(p)) + p + struct.pack("<I", len(v)) + v + struct.pack("<I", n_bn) + bn + g2 + struct.pack("<I", n_gr) + gr
open(out, "wb").write(body)
