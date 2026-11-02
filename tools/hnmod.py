#!/usr/bin/env python3
"""Bundle, check and package the native mods of this repository.

  python3 tools/hnmod.py bundle  mods/<id>   # native/horizon.rs + src/lib.rs -> mixins/<id>/runtime.rs
  python3 tools/hnmod.py check   mods/<id>   # the manifest's pin, and that runtime.rs is up to date
  python3 tools/hnmod.py package mods/<id>   # -> mods/<id>/target/<id>-<version>.hnmod (reproducible)

A mod folder holds a Cargo crate with the mod's rules (`src/lib.rs`, tested alone with `cargo test`),
the adapter `native/horizon.rs` (it contains exactly `include!("<id>_engine.rs");`, where the rules go)
and the horizon-mixins bundle `mixins/<id>/` (`horizon-mixin.toml` and its helper files). Only the
`.rs` and `.toml` files of `mixins/<id>/` go into the `.hnmod`, with `horizon-native.json`.

Any line `include!("kit_<name>.rs");` of the adapter is replaced by `tools/kit/kit_<name>.rs` (helpers
several mods share). Some mods have extra bundles made by their own `tools/bundle_*.py`.

The pins can be changed with HORIZON_PIN and HORIZON_MIXINS. The archive has fixed dates and order, so
the same sources always give the same SHA-256.
"""
import hashlib
import json
import os
import re
import sys
import zipfile
from pathlib import Path

HORIZON = os.environ.get("HORIZON_PIN", "142388e1c136e622652ec7505af15d8ad195e041")
MIXINS = os.environ.get("HORIZON_MIXINS", "0d6b103c70633d8fd9443d8d0acd3aadaacedb06")
KIT = Path(__file__).resolve().parent / "kit"
KIT_LINE = re.compile(r'include!\("(kit_[a-z0-9_]+)\.rs"\);')


def folder(arg):
    d = Path(arg).resolve()
    mod = d.name
    if not (d / "mixins" / mod / "horizon-mixin.toml").is_file():
        raise SystemExit(f"{d}: no mixins/{mod}/horizon-mixin.toml")
    return d, mod


def kit(adapter):
    def one(m):
        f = KIT / f"{m.group(1)}.rs"
        if not f.is_file():
            raise SystemExit(f"unknown kit file {f.name}")
        return f.read_text(encoding="utf-8").rstrip("\n")
    return KIT_LINE.sub(one, adapter)


def payload(d, mod):
    adapter = (d / "native" / "horizon.rs").read_text(encoding="utf-8")
    marker = f'include!("{mod}_engine.rs");'
    if marker not in adapter:
        raise SystemExit(f"native/horizon.rs must contain {marker}")
    return kit(adapter).replace(marker, (d / "src" / "lib.rs").read_text(encoding="utf-8"))


def manifest(d, mod):
    toml = (d / "mixins" / mod / "horizon-mixin.toml").read_text(encoding="utf-8")
    if f'horizon_revision = "{HORIZON}"' not in toml:
        raise SystemExit(f"horizon_revision must be {HORIZON}")
    if not re.search(rf'^id = "{re.escape(mod)}"', toml, re.M):
        raise SystemExit(f'[mod] id must be "{mod}"')
    version = re.search(r'^version = "([^"]+)"', toml, re.M).group(1)
    return dict(schema=1, id=mod, version=version, game="26.3", horizon=HORIZON, mixins=MIXINS)


def bundle(arg):
    d, mod = folder(arg)
    out = d / "mixins" / mod / "runtime.rs"
    out.write_text(payload(d, mod), encoding="utf-8", newline="\n")
    print(f"{out} ({out.stat().st_size} B)")


def check(arg):
    d, mod = folder(arg)
    m = manifest(d, mod)
    rt = d / "mixins" / mod / "runtime.rs"
    if rt.read_text(encoding="utf-8") != payload(d, mod):
        raise SystemExit("runtime.rs is not up to date: run bundle")
    print(json.dumps(m))


def package(arg):
    check(arg)
    d, mod = folder(arg)
    m = manifest(d, mod)
    files = {"horizon-native.json": (json.dumps(m, indent=2, sort_keys=True) + "\n").encode()}
    for p in sorted((d / "mixins" / mod).iterdir()):
        if p.suffix in (".rs", ".toml"):
            files[f"mixins/{mod}/{p.name}"] = p.read_bytes()
    out = d / "target" / f"{mod}-{m['version']}.hnmod"
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for name, content in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            z.writestr(info, content)
    data = out.read_bytes()
    print(json.dumps(dict(file=str(out), size=len(data), sha256=hashlib.sha256(data).hexdigest())))


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("bundle", "package", "check"):
        raise SystemExit(__doc__)
    globals()[sys.argv[1]](sys.argv[2])
