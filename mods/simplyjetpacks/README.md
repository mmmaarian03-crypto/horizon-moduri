# Simply Jetpacks 2 for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust): an unofficial port of **Simply Jetpacks 2 2.11.0.1** (Forge 1.20.1) by Tomson124, StormedPanda. It is not made or endorsed by the original's authors or by the Horizon project.

Jetpacks and flux packs: flight with fuel energy, hover modes, keys and a HUD.

## Differences from the original

- Nothing charges other mods' items yet, so a jetpack only fills where a machine supports it.
- The jetpack is not drawn on the back; no jetpack screen, loop sound or custom recipes.
- Settings the original reads from its config file are fixed in code, with the values of the 1.20.1 pack this port was made for (the original's defaults unless the source says otherwise).

## Original

- Project: https://github.com/Tomson124/SimplyJetpacks2 (Modrinth: https://modrinth.com/mod/simply-jetpacks-2)
- Authors: Tomson124, StormedPanda
- Code licence: `MIT`. Assets of the original: same licence.
- How this port was written: Written from the decompiled 1.20.1 jar (Fernflower), under the original's MIT licence.
- This folder holds only the port's Rust source. It contains no code, textures, models, sounds, translations or data files from the original's jar.

## Content

The blocks, items, textures, sounds and translations come from **your own copy** of the original 1.20.1 jar: Horizon's modpack porter (`horizon-port-pack`, see Horizon's docs/modpacks.md) turns it into a content mod on your machine. Neither this repository nor the `.hnmod` file ships any of it. Without that content the mod's blocks and items do not exist in the game.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/simplyjetpacks/horizon-mixin.toml`).

1. Get `simplyjetpacks-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/simplyjetpacks`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/simplyjetpacks --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

Needs in the same build: `foundry` (mods/foundry).

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

`MIT`, the original's licence (`LICENSE`). The original is copyright its authors; this port is copyright 2026 Marian and is licensed under the same terms.
