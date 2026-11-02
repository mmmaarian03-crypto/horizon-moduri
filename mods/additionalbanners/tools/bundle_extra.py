#!/usr/bin/env python3
"""Writes mixins/additionalbanners/content_runtime.rs and menu_runtime.rs: native/content.rs and native/menu.rs
with src/lib.rs in place of their include! (tools/hnmod.py's bundle makes runtime.rs only). Run it before
tools/hnmod.py:  py -I -X utf8 tools/bundle_extra.py"""
from pathlib import Path

root = Path(__file__).resolve().parents[1]
marker = 'include!("additionalbanners_engine.rs");'
engine = (root / "src" / "lib.rs").read_text(encoding="utf-8")
for src, out in (("content.rs", "content_runtime.rs"), ("menu.rs", "menu_runtime.rs")):
    text = (root / "native" / src).read_text(encoding="utf-8")
    assert marker in text, src
    dest = root / "mixins" / "additionalbanners" / out
    dest.write_text(text.replace(marker, engine), encoding="utf-8", newline="\n")
    print(dest, dest.stat().st_size)
