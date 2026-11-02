# Quick start for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust).

Makes the start of a game with many content mods faster on Windows: the base files of the content root are hard-linked instead of copied, with the same result byte for byte.

## Notes

- Not a port: there is no original.
- The base files become second links of the files they come from, so deleting a build does not free their space while the cache keeps them.
- Resource packs are not touched.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/pornire_rapida/horizon-mixin.toml`).

1. Get `pornire_rapida-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/pornire_rapida`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/pornire_rapida --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

MIT, copyright 2026 Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). See `LICENSE`.
