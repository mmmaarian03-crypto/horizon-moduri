# Cave Dust for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust): an unofficial port of **Cave Dust 2.0.4** (Forge 1.20.1) by LizIsTired. It is not made or endorsed by the original's authors or by the Horizon project.

Dust particles floating in caves, denser the deeper you are.

## Differences from the original

- The biome is read from the chunk without vanilla's biome zoom (differs only at borders).
- The particles use their own random generator so they never touch the world's random sequences.
- Settings the original reads from its config file are fixed in code, with the values of the 1.20.1 pack this port was made for (the original's defaults unless the source says otherwise).

## Original

- Project: https://github.com/LizIsTired/cave_dust (Modrinth: https://modrinth.com/mod/cave-dust)
- Authors: LizIsTired
- Code licence: `MPL-2.0`. Assets of the original: same licence.
- How this port was written: Written from the public source.
- This folder holds only the port's Rust source. It contains no code, textures, models, sounds, translations or data files from the original's jar.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/cavedust/horizon-mixin.toml`).

1. Get `cavedust-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/cavedust`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/cavedust --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

`MPL-2.0`, the original's licence (`LICENSE`). The original is copyright its authors; this port is copyright 2026 Marian and is licensed under the same terms.

As that licence asks, the port is published as source under the same licence, with the original credited and linked above.
