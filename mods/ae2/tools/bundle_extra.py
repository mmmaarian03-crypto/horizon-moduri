#!/usr/bin/env python3
"""Writes mixins/ae2/worldgen_runtime.rs (native/worldgen.rs with src/lib.rs pasted in place of
its include!). tools/hnmod.py's bundle makes runtime.rs only; run this before tools/hnmod.py.
  py -I -X utf8 mods/ae2/tools/bundle_extra.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("ae2_engine.rs");'
engine = (root / "src" / "lib.rs").read_text(encoding="utf-8")
text = (root / "native" / "worldgen.rs").read_text(encoding="utf-8")
out = root / "mixins" / "ae2" / "worldgen_runtime.rs"
out.write_text(text.replace(marker, engine), encoding="utf-8", newline="\n")
print(out, out.stat().st_size)
