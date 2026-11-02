# Forja for Horizon

A native mod for [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition 26.3 rewritten in Rust).

A shared foundation for the other native mods: machines and block entities kept in the mod's own save file, item, fluid and energy handlers, multipart blocks, menus that also work for LAN guests and server players, and a small network layer (named channels) between client and server.

## Notes

- Not a port: there is no original. Other native mods of this repository call it as `crate::game::forja`: its API is in `API.md`.
- Blocks with several parts: rotated model elements (22.5/45 degrees) are kept unrotated, and models larger than the block are clipped to it.
- Strong redstone signal only towards the block below; no facades, no Mekanism gases or AE2 networks (those mods add their own).

## Content

This mod needs no content files: it works with the game's own resources.

## Build and install

Pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06` (`mixins/forja/horizon-mixin.toml`).

1. Get `forja-0.1.0.hnmod`: from Horizon's registry once it is listed, or build it at the root of this repository with `python3 tools/hnmod.py package mods/forja`.
2. In the Horizon launcher: **Mods**, install the file (the first native mod shows a notice and turns on Settings -> Game -> **Native mods (experimental)**), then **Build** and confirm the red warning. From a terminal: `horizon-launcher native-build mods/forja --instance <name> --trust-native-code`.
3. Play the instance the launcher built.

The rules have their own tests, without the game: `cargo test` in this folder. Some tests also read your generated content when it is in `content/` at the root of this repository, and skip otherwise; the ones that cannot run without it are marked `#[ignore]` (`cargo test -- --ignored` once the content is there).

## Made with AI

Ported with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). It was checked with the tests in `tests/` and by compiling the game with the mod applied; not every feature has been played in game yet.

## Licence

MIT, copyright 2026 Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)). See `LICENSE`.
