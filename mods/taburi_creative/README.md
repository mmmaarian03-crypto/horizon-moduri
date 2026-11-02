# Creative tabs for mods for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust).

Adds one creative inventory tab per installed content mod, with pages like Forge's, so modded blocks and items show up in the creative inventory and its search.

## Notes

- One tab per mod (Forge mods may have several).
- Items in a tab follow Horizon's registry order; the tab icon is the first item; the title is the mod's English name.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/taburi_creative/horizon-mixin.toml`).

1. Get `taburi_creative-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/taburi_creative`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/taburi_creative --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

MIT, copyright 2026 Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). See `LICENSE`.
