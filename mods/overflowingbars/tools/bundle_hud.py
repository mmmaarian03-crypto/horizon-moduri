#!/usr/bin/env python3
"""Writes mixins/overflowingbars/hud_runtime.rs: native/hud.rs with src/lib.rs in place of its include!
(tools/hnmod.py's bundle makes runtime.rs only). Run it before tools/hnmod.py:
    py -I -X utf8 tools/bundle_hud.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("overflowingbars_engine.rs");'
text = (root / "native" / "hud.rs").read_text(encoding="utf-8")
assert marker in text
out = root / "mixins" / "overflowingbars" / "hud_runtime.rs"
out.write_text(text.replace(marker, (root / "src" / "lib.rs").read_text(encoding="utf-8")),
               encoding="utf-8", newline="\n")
print(out, out.stat().st_size)
