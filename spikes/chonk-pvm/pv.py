# The proof and vk alone, [u32 proof_len][proof][u32 vk_len][vk], for chonk_pvm_verify_crs (aztec-on-jam#73).
#   pv.py <proof> <vk> <out> [flip_offset]
import sys, struct
p = bytearray(open(sys.argv[1], "rb").read()); v = open(sys.argv[2], "rb").read()
if len(sys.argv) > 4: p[int(sys.argv[4])] ^= 1
open(sys.argv[3], "wb").write(struct.pack("<I", len(p)) + p + struct.pack("<I", len(v)) + v)
