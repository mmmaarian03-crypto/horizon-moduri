# shared.rs (the part in the game library) = src/lib.rs + native/library.rs
from pathlib import Path

root = Path(__file__).resolve().parents[1]
text = (root / "src/lib.rs").read_text(encoding="utf-8") + (root / "native/library.rs").read_text(encoding="utf-8")
out = root / "mixins/enderstorage/shared.rs"
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(text, encoding="utf-8", newline="\n")
print(out, len(text))
