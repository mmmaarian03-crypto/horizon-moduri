#!/usr/bin/env python3
"""Writes mixins/hangglider/items_runtime.rs: native/items.rs with src/lib.rs in place of its include!
(tools/hnmod.py's bundle makes runtime.rs only). Run it before tools/hnmod.py:
    py -I -X utf8 tools/bundle_extra.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("hangglider_engine.rs");'
text = (root / "native" / "items.rs").read_text(encoding="utf-8")
assert marker in text
out = root / "mixins" / "hangglider" / "items_runtime.rs"
out.write_text(text.replace(marker, (root / "src" / "lib.rs").read_text(encoding="utf-8")),
               encoding="utf-8", newline="\n")
print(out, out.stat().st_size)
