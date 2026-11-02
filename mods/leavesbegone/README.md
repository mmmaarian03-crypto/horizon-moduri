# Leaves Be Gone for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust): an unofficial port of **Leaves Be Gone 8.0.0** (Forge 1.20.1) by Fuzs. It is not made or endorsed by the original's authors or by the Horizon project.

Leaves decay within a second once their tree is cut, instead of minutes.

## Differences from the original

- The decay queue is not saved in chunks: leaving the world within 19 ticks of a cut leaves those leaves to vanilla decay.
- Does nothing on someone else's server (the rule runs on the server).
- Settings the original reads from its config file are fixed in code, with the values of the 1.20.1 pack this port was made for (the original's defaults unless the source says otherwise).

## Original

- Project: https://github.com/Fuzss/leavesbegone (Modrinth: https://modrinth.com/mod/leaves-be-gone)
- Authors: Fuzs
- Code licence: `MPL-2.0`. Assets of the original: All Rights Reserved (none used here).
- How this port was written: Written from the public source.
- This folder holds only the port's Rust source. It contains no code, textures, models, sounds, translations or data files from the original's jar.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/leavesbegone/horizon-mixin.toml`).

1. Get `leavesbegone-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/leavesbegone`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/leavesbegone --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

`MPL-2.0`, the original's licence (`LICENSE`, `LICENSE-ASSETS.md`). The original is copyright its authors; this port is copyright 2026 Marian and is licensed under the same terms.

As that licence asks, the port is published as source under the same licence, with the original credited and linked above.
