# Clumps for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust): an unofficial port of **Clumps 12.0.0.4** (Forge 1.20.1) by Jaredlll08. It is not made or endorsed by the original's authors or by the Horizon project.

Experience orbs near each other merge into one, so many orbs cost less and are picked up at once.

## Differences from the original

- The merged orb's map of values is not saved with the world (the orb loader cannot be hooked).
- Mending uses the game's own 26.3 repair rule.
- Settings the original reads from its config file are fixed in code, with the values of the 1.20.1 pack this port was made for (the original's defaults unless the source says otherwise).

## Original

- Project: https://github.com/jaredlll08/Clumps (Modrinth: https://modrinth.com/mod/clumps)
- Authors: Jaredlll08
- Code licence: `MIT`. Assets of the original: same licence.
- How this port was written: Written from the public source.
- This folder holds only the port's Rust source. It contains no code, textures, models, sounds, translations or data files from the original's jar.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/clumps/horizon-mixin.toml`).

1. Get `clumps-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/clumps`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/clumps --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

`MIT`, the original's licence (`LICENSE`). The original is copyright its authors; this port is copyright 2026 Marian and is licensed under the same terms.
