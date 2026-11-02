#!/usr/bin/env python3
"""Writes the extra module of the bundle (native/worldgen.rs) into mixins/enderore/worldgen_runtime.rs,
with src/lib.rs in place of its include! (tools/hnmod.py's bundle makes runtime.rs only). Run it before
tools/hnmod.py:   py -I -X utf8 tools/bundle_extra.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("enderore_engine.rs");'
engine = (root / "src" / "lib.rs").read_text(encoding="utf-8")
text = (root / "native" / "worldgen.rs").read_text(encoding="utf-8")
out = root / "mixins" / "enderore" / "worldgen_runtime.rs"
out.write_text(text.replace(marker, engine) if marker in text else text, encoding="utf-8", newline="\n")
print(out, out.stat().st_size)
