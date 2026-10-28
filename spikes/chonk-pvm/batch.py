# A batch for chonk_pvm_verify_batch: [u32 n] then each [u32 proof_len][proof][u32 vk_len][vk] (pv.py's files).
#   batch.py <out> <pv file>...
import sys, struct
items = [open(f, "rb").read() for f in sys.argv[2:]]
open(sys.argv[1], "wb").write(struct.pack("<I", len(items)) + b"".join(items))
