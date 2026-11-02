#!/usr/bin/env python3
"""mixins/easyanvils/shared.rs = src/lib.rs + tools/glue.rs (the module put into horizon_client::stations)."""
import pathlib

R = pathlib.Path(__file__).resolve().parents[1]
t = (R / "src" / "lib.rs").read_text(encoding="utf-8") + (R / "tools" / "glue.rs").read_text(encoding="utf-8")
(R / "mixins" / "easyanvils" / "shared.rs").write_text(t, encoding="utf-8", newline="\n")
print("shared.rs", len(t))
