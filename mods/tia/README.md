# Tiny Item Animations for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust): an unofficial port of **Tiny Item Animations 1.1** (Forge 1.20.1) by Trivaxy. It is not made or endorsed by the original's authors or by the Horizon project.

Small animations when items enter or leave a slot.

## Differences from the original

- The scale is applied to the item's vertices in the HUD batch (the original scales the draw matrix).
- The snap-back of a cancelled drag is not animated.
- Settings the original reads from its config file are fixed in code, with the values of the 1.20.1 pack this port was made for (the original's defaults unless the source says otherwise).

## Original

- Project: https://github.com/Trivaxy/tiny-item-animations (Modrinth: https://modrinth.com/mod/tiny-item-animations)
- Authors: Trivaxy
- Code licence: `MIT`. Assets of the original: same licence.
- How this port was written: Written from the public source (branch forge-1.20, at the MIT-licensed 1.1 version).
- This folder holds only the port's Rust source. It contains no code, textures, models, sounds, translations or data files from the original's jar.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/tia/horizon-mixin.toml`).

1. Get `tia-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/tia`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/tia --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

`MIT`, the original's licence (`LICENSE`). The original is copyright its authors; this port is copyright 2026 Marian and is licensed under the same terms.
