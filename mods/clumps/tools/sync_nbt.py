#!/usr/bin/env python3
"""Writes mixins/clumps/nbt.rs: the block of src/lib.rs between the two `// ---` markers
(the part of the rules the save needs). Run it after changing that block; tests/save.rs checks
the two stay the same."""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
lib = (root / "src" / "lib.rs").read_text(encoding="utf-8")
begin = lib.index("// --- the part of the rules the save needs")
begin = lib.index("\n", begin) + 1
end = lib.index("// --- end of the save block ---")
block = lib[begin:end]
out = root / "mixins" / "clumps" / "nbt.rs"
out.write_text(block, encoding="utf-8", newline="\n")
print(f"{out} ({len(block)} B)")
