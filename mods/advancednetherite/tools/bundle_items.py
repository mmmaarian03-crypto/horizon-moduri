#!/usr/bin/env python3
"""Writes mixins/advancednetherite/items_runtime.rs: native/items.rs with src/lib.rs
in place of its include!. Run it before tools/hnmod.py (tools/hnmod.py's bundle makes
runtime.rs only).  py -I -X utf8 tools/bundle_items.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("advancednetherite_engine.rs");'
items = (root / "native" / "items.rs").read_text(encoding="utf-8")
assert marker in items
out = root / "mixins" / "advancednetherite" / "items_runtime.rs"
out.write_text(items.replace(marker, (root / "src" / "lib.rs").read_text(encoding="utf-8")),
               encoding="utf-8", newline="\n")
print(out, out.stat().st_size)
