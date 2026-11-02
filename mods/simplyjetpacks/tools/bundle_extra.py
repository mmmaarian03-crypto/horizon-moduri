#!/usr/bin/env python3
"""Writes mixins/simplyjetpacks/content_runtime.rs: native/content.rs with src/lib.rs in place of its include!
(tools/hnmod.py's bundle makes runtime.rs only). Run it before tools/hnmod.py:  py -I -X utf8 tools/bundle_extra.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("simplyjetpacks_engine.rs");'
engine = (root / "src" / "lib.rs").read_text(encoding="utf-8")
text = (root / "native" / "content.rs").read_text(encoding="utf-8")
assert marker in text
dest = root / "mixins" / "simplyjetpacks" / "content_runtime.rs"
dest.write_text(text.replace(marker, engine), encoding="utf-8", newline="\n")
print(dest, dest.stat().st_size)
