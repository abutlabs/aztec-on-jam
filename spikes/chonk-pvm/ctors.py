"""The PVM has no startup code, so .init_array never runs. In one file's LLVM IR, replace
@llvm.global_ctors with an exported chonk_ctors_<id>() that calls its constructors in priority
order. A constructor in a comdat (an inline variable's guarded initializer) leaves the comdat,
so no caller points into a copy the linker discards; its guard keeps it to one run.
Usage: ctors.py FILE.ll ID   (rewrites FILE.ll; prints ID if the file had constructors)"""
import re, sys
path, ident = sys.argv[1], sys.argv[2]
lines = open(path).read().split("\n")
idx = [i for i, l in enumerate(lines) if l.startswith("@llvm.global_ctors = ")]
if not idx:
    if any(l.startswith(f"define void @chonk_ctors_{ident}()") for l in lines):
        print(ident)  # rewritten by an earlier run
    sys.exit(0)
entries = re.findall(r"\{ i32 (\d+), ptr (@[^,\s]+), ptr [^}]*\}", lines[idx[0]])
names = [n for _, n in sorted(entries, key=lambda e: int(e[0]))]
del lines[idx[0]]
for i, l in enumerate(lines):
    if l.startswith("define ") and any(f" {n}(" in l for n in names):
        lines[i] = re.sub(r" comdat(\([^)]*\))?", "", l)
lines.append(f"define void @chonk_ctors_{ident}() {{")
lines += [f"  call void {n}()" for n in names]
lines += ["  ret void", "}"]
open(path, "w").write("\n".join(lines))
print(ident)
