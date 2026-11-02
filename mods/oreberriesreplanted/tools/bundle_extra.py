#!/usr/bin/env python3
"""Writes the three extra modules of the bundle (content.rs, recipes.rs, worldgen.rs of native/)
into mixins/oreberriesreplanted/<name>_runtime.rs, each with src/lib.rs in place of its
include!. Run it before tools/hnmod.py (tools/hnmod.py's bundle makes runtime.rs only).
  py -I -X utf8 tools/bundle_extra.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("oreberriesreplanted_engine.rs");'
engine = (root / "src" / "lib.rs").read_text(encoding="utf-8")
for name in ("content", "worldgen"):
    text = (root / "native" / f"{name}.rs").read_text(encoding="utf-8")
    out = root / "mixins" / "oreberriesreplanted" / f"{name}_runtime.rs"
    out.write_text(text.replace(marker, engine) if marker in text else text, encoding="utf-8", newline="\n")
    print(out, out.stat().st_size)
