#!/usr/bin/env python3
"""The client's empty trees are the privacy service's genesis: the note root and nullifier
root it starts from when its storage is empty (services/privacy/src/lib.rs, EMPTY_NOTE_ROOT and
EMPTY_NULL_ROOT). If these drift, the first proof on a fresh chain is refused (or, before the
constants existed, could bring a tree of its own)."""
import re, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from privacy import Tree, IndexedTree  # noqa: E402

src = (HERE.parent / "services" / "privacy" / "src" / "lib.rs").read_text()
want = {k: int(v, 16) for k, v in re.findall(r'const (EMPTY_NOTE_ROOT|EMPTY_NULL_ROOT): \[u8; 32\] = hex32\("([0-9a-f]{64})"\)', src)}
assert set(want) == {"EMPTY_NOTE_ROOT", "EMPTY_NULL_ROOT"}, want
got = {"EMPTY_NOTE_ROOT": Tree([]).root(), "EMPTY_NULL_ROOT": IndexedTree().root()}
for k in want:
    assert got[k] == want[k], f"{k}: the client's {got[k]:#066x}, the service's {want[k]:#066x}"
print("genesis: the client's empty trees are the service's: note root %s…, nullifier root %s…"
      % (f"{want['EMPTY_NOTE_ROOT']:064x}"[:12], f"{want['EMPTY_NULL_ROOT']:064x}"[:12]))
