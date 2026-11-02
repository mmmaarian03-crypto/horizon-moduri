# Ender Ore (rebuild) for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust).

Ender ore in stone, deepslate and end stone: it drops ender pearls, with its own world generation.

## Notes

- The order of the features in their generation step follows Horizon's, so ore positions differ from Forge's for the same seed.

## Content

The blocks, items, textures, sounds and translations come from **your own copy** of the original 1.20.1 jar: Horizon's modpack porter (`horizon-port-pack`, see Horizon's docs/modpacks.md) turns it into a content mod on your machine. Neither this repository nor the `.hnmod` file ships any of it. Without that content the mod's blocks and items do not exist in the game.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/enderore/horizon-mixin.toml`).

1. Get `enderore-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/enderore`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/enderore --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

MIT, copyright 2026 Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). See `LICENSE`.
