# Loading screen for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust).

A loading screen while Horizon prepares the content of many mods (instead of a black window): the Horizon logo, a progress bar with how many mods are ready, and the memory in use.

## Notes

- Not a port: there is no original.
- The texts are English. For Romanian ones, set `ROMANIAN` to `true` in `src/lib.rs` and in `mixins/ecran_incarcare/shared.rs` (the font draws Romanian's accented letters).
- The logo is a 48x48 reduction of Horizon's own icon (`packaging/icons`, MIT, Ludovic Marie and the Horizon contributors).
- Text is upper case only (a 5x7 font); a character without a glyph is skipped.

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/ecran_incarcare/horizon-mixin.toml`).

1. Get `ecran_incarcare-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/ecran_incarcare`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/ecran_incarcare --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise.

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

MIT, copyright 2026 Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). See `LICENSE`.
