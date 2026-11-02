# Horizon native mods

Native mods for [Horizon](https://github.com/horizon-rust/horizon), Minecraft: Java Edition 26.3 rewritten in Rust.
Most of them are unofficial ports of Forge 1.20.1 mods whose licence allows a port to be shared; a few are original.
None of them is made or endorsed by the original mods' authors or by the Horizon project.

**What is here: source code only.** No textures, models, sounds, translations or data files of the original mods are in
this repository or in the `.hnmod` files built from it. Each player makes that content on their own machine from their
own copy of the original jar (see [Content](#content)).

Made with AI agents (Claude, by Anthropic) for Marian ([mmmaarian03-crypto](https://github.com/mmmaarian03-crypto)), with tests for the rules of every
mod. Not every feature has been played in game yet: each mod's README says what differs from the original.

## The mods

`mods/` holds 47 mods. `mods-nc/` holds 4 ports of
mods under **non-commercial** licences: they may be shared and changed, never sold or used to make money (see
[LICENSE-NOTES.md](LICENSE-NOTES.md)).

| Mod | Original | Licence | Needs |
|---|---|---|---|
| [`creative_tabs`](mods/creative_tabs) | original (no Java mod) | `MIT` |  |
| [`enderore`](mods/enderore) | the author's own 1.20.1 rebuild | `MIT` |  |
| [`fast_start`](mods/fast_start) | original (no Java mod) | `MIT` |  |
| [`foundry`](mods/foundry) | original (no Java mod) | `MIT` |  |
| [`loading_screen`](mods/loading_screen) | original (no Java mod) | `MIT` |  |
| [`netherending`](mods/netherending) | the author's own 1.20.1 rebuild | `MIT` |  |
| [`a_good_place`](mods/a_good_place) | [A Good Place 1.2.8](https://github.com/enjarai/a-good-place) by MehVahdJukaar, enjarai | `MIT` |  |
| [`additionalbanners`](mods/additionalbanners) | [Additional Banners 14.0.5](https://github.com/Darkhax-Minecraft/AdditionalBanners) by Darkhax | `LGPL-2.1-only` |  |
| [`advancednetherite`](mods/advancednetherite) | [Advanced Netherite 2.1.3](https://github.com/Autovw/AdvancedNetherite) by Autovw | `GPL-3.0-only` |  |
| [`ae2`](mods/ae2) | [Applied Energistics 2 15.4.11](https://github.com/AppliedEnergistics/Applied-Energistics-2) by AlgorithmX2, TeamAppliedEnergistics | `LGPL-3.0-only` | foundry |
| [`attributefix`](mods/attributefix) | [AttributeFix 21.0.5](https://github.com/Darkhax-Minecraft/AttributeFix) by Darkhax | `LGPL-2.1-only` |  |
| [`betteradvancements`](mods-nc/betteradvancements) | [Better Advancements 0.6.0.73](https://github.com/way2muchnoise/BetterAdvancements) by way2muchnoise | `LicenseRef-DBaJ-NC` |  |
| [`capsule`](mods/capsule) | [Capsule 8.0.105](https://github.com/Lythom/capsule) by Lythom | `MIT` |  |
| [`carryon`](mods/carryon) | [Carry On 2.1.2.7](https://github.com/Tschipp/CarryOn) by Tschipp, Purplicious_Cow, cy4n | `LGPL-3.0-only` | foundry |
| [`cavedust`](mods/cavedust) | [Cave Dust 2.0.4](https://github.com/LizIsTired/cave_dust) by LizIsTired | `MPL-2.0` |  |
| [`clumps`](mods/clumps) | [Clumps 12.0.0.4](https://github.com/jaredlll08/Clumps) by Jaredlll08 | `MIT` |  |
| [`comforts`](mods/comforts) | [Comforts 6.4.0](https://github.com/illusivesoulworks/comforts) by Illusive Soulworks | `LGPL-3.0-or-later` |  |
| [`curios`](mods/curios) | [Curios API 5.14.1](https://github.com/TheIllusiveC4/Curios) by C4, Illusive Soulworks | `LGPL-3.0-or-later` | foundry |
| [`waterdripsound`](mods/waterdripsound) | [Drip Sounds 0.6.3](https://github.com/PieKing1215/DripSounds) by PieKing1215 | `LGPL-3.0-only` |  |
| [`easyanvils`](mods/easyanvils) | [Easy Anvils 8.0.2](https://github.com/Fuzss/easyanvils) by Fuzs | `MPL-2.0` | foundry |
| [`enchantinginfuser`](mods/enchantinginfuser) | [Enchanting Infuser 8.0.3](https://github.com/Fuzss/enchantinginfuser) by Fuzs | `MPL-2.0` | foundry |
| [`enchdesc`](mods/enchdesc) | [Enchantment Descriptions 17.1.21](https://github.com/Darkhax-Minecraft/Enchantment-Descriptions) by Darkhax | `LGPL-2.1-only` |  |
| [`enderstorage`](mods/enderstorage) | [EnderStorage 2.11.0.188](https://github.com/TheCBProject/EnderStorage) by ChickenBones, covers1624 | `MIT` | foundry |
| [`escape_rope`](mods/escape_rope) | [Escape Rope 1.0.0](https://modrinth.com/mod/escape-rope) by rdani | `AFL-3.0` |  |
| [`explorerscompass`](mods-nc/explorerscompass) | [Explorer's Compass 1.4.0](https://github.com/MattCzyr/ExplorersCompass) by ChaosTheDude (MattCzyr) | `CC-BY-NC-SA-4.0` | foundry |
| [`fallingtree`](mods/fallingtree) | [FallingTree 4.3.4](https://github.com/RakambdaOrg/FallingTree) by Rakambda | `LGPL-3.0-only` |  |
| [`hangglider`](mods/hangglider) | [Hang Glider 8.0.1](https://github.com/Fuzss/hangglider) by gr8pefish, Fuzs | `MIT` |  |
| [`hats`](mods/hats) | [Hats Renewed 20.1.2](https://github.com/guivnf/Hats-renewed) by iChun, guivnf | `LGPL-3.0-only` |  |
| [`ironchest`](mods/ironchest) | [Iron Chests 14.4.4](https://github.com/progwml6/ironchest) by cpw, alexbegt, progwml6 | `GPL-3.0-only` | foundry |
| [`itemphysic`](mods/itemphysic) | [ItemPhysic 1.8.17](https://github.com/CreativeMD/ItemPhysic) by CreativeMD | `LGPL-3.0-only` |  |
| [`leavesbegone`](mods/leavesbegone) | [Leaves Be Gone 8.0.0](https://github.com/Fuzss/leavesbegone) by Fuzs | `MPL-2.0` |  |
| [`modonomicon`](mods/modonomicon) | [Modonomicon 1.79.3](https://github.com/klikli-dev/modonomicon) by klikli-dev | `MIT` |  |
| [`morebows`](mods/morebows) | [More Bows Restrung 1.0.13](https://github.com/NeRdTheNed/More-bows) by GaussFire, iDiamondhunter, NeRd | `CC0-1.0` |  |
| [`moreoverlays`](mods/moreoverlays) | [More Overlays Updated 1.22.7](https://github.com/r8420/MoreOverlays-updated) by feldim2425, RiDGo8/R8420 | `MIT` |  |
| [`mousetweaks`](mods/mousetweaks) | [Mouse Tweaks 2.25.1](https://github.com/YaLTeR/MouseTweaks) by Ivan Molodetskikh (YaLTeR) | `BSD-3-Clause` |  |
| [`naturescompass`](mods-nc/naturescompass) | [Nature's Compass 1.12.0](https://github.com/MattCzyr/NaturesCompass) by ChaosTheDude (MattCzyr) | `CC-BY-NC-SA-4.0` | foundry |
| [`neat`](mods/neat) | [Neat 41](https://github.com/VazkiiMods/Neat) by Vazkii, Violet Moon | `MIT` |  |
| [`oreberriesreplanted`](mods/oreberriesreplanted) | [Oreberries Replanted 0.5.2](https://github.com/Mrbysco/OreBerries-Replanted) by Mrbysco, ShyNieke | `MIT` |  |
| [`overflowingbars`](mods/overflowingbars) | [Overflowing Bars 8.0.1](https://github.com/Fuzss/overflowingbars) by Fuzs | `MPL-2.0` |  |
| [`patchouli`](mods-nc/patchouli) | [Patchouli 1.20.1-85](https://github.com/VazkiiMods/Patchouli) by Vazkii, Violet Moon | `CC-BY-NC-SA-3.0` |  |
| [`polymorph`](mods/polymorph) | [Polymorph 0.49.10](https://github.com/illusivesoulworks/polymorph) by Illusive Soulworks | `LGPL-3.0-or-later` | foundry |
| [`presencefootsteps`](mods/presencefootsteps) | [Presence Footsteps 1.0.0 (Forge port)](https://github.com/Sollace/Presence-Footsteps) by Hurricaaane (Ha3), Sollace, 1Influence | `MIT` |  |
| [`simplyjetpacks`](mods/simplyjetpacks) | [Simply Jetpacks 2 2.11.0.1](https://github.com/Tomson124/SimplyJetpacks2) by Tomson124, StormedPanda | `MIT` | foundry |
| [`smoothswapping`](mods/smoothswapping) | [Smooth Swapping 0.9.3.2](https://github.com/Schauweg/Smooth-Swapping) by Schauweg, Riflusso | `LGPL-3.0-or-later` |  |
| [`soulshards`](mods/soulshards) | [Soul Shards 1.3.6](https://github.com/CreeperHost/SoulShards) by TehNut, CreeperHost | `MIT` |  |
| [`storagedrawers`](mods/storagedrawers) | [Storage Drawers 12.15.1](https://github.com/jaquadro/StorageDrawers) by jaquadro (Texelsaur) | `MIT` |  |
| [`tlc`](mods/tlc) | [The Lost Castle 1.0.3](https://github.com/Team-Remastered/The-Lost-Castle-Forge) by Team Remastered | `LGPL-3.0-only` |  |
| [`torchmaster`](mods/torchmaster) | [Torchmaster 20.1.9](https://github.com/Xalcon/TorchMaster) by Xalcon | `MIT` |  |
| [`travelerstitles`](mods/travelerstitles) | [Traveler's Titles 4.0.2](https://github.com/YUNG-GANG/Travelers-Titles) by YUNGNICKYOUNG | `LGPL-3.0-only` |  |
| [`visualworkbench`](mods/visualworkbench) | [Visual Workbench 8.0.1](https://github.com/Fuzss/visualworkbench) by Fuzs | `MPL-2.0` | foundry |
| [`yungsbridges`](mods/yungsbridges) | [YUNG's Bridges 4.0.3](https://github.com/YUNG-GANG/YUNGs-Bridges) by YUNGNICKYOUNG | `LGPL-3.0-only` |  |

## Build and install

Every mod is pinned to Horizon `142388e1c136e622652ec7505af15d8ad195e041` and horizon-mixins `0d6b103c70633d8fd9443d8d0acd3aadaacedb06`. All the mods of one build must
pin the same Horizon commit (Horizon's docs/native-mods.md).

1. Make the `.hnmod` of a mod (or take it from Horizon's registry once it is listed there):
   `python3 tools/hnmod.py package mods/<id>` writes `mods/<id>/target/<id>-<version>.hnmod`. The archive is
   reproducible: the same sources give the same SHA-256.
2. In the Horizon launcher, **Mods**: install the file. The first native mod shows a notice and turns on Settings ->
   Game -> **Native mods (experimental)**. Then **Build**, read the red warning and confirm. The first build downloads
   Rust and compiles the game (20 to 30 minutes); later builds are faster.
3. From a terminal instead: `horizon-launcher native-build <folder> --instance <name> --trust-native-code`, where the
   folder is a mod folder or a folder of them.

A mod that needs `foundry` must be built together with `mods/foundry`.

Each mod folder is also a small Cargo crate with the mod's rules: `cargo test` in it runs their tests without the game.

`tools/hnmod.py bundle mods/<id>` rebuilds `mixins/<id>/runtime.rs` from `native/horizon.rs` and `src/lib.rs` after a
change (some mods have more bundles, made by their own `tools/bundle_*.py`); `check` says whether it is up to date.

## Content

Ported mods that add blocks, items, textures or sounds need a **content mod made from your own copy of the original
1.20.1 jar**. Horizon's modpack porter does that on your machine (Horizon's docs/modpacks.md):

```sh
cargo run -p horizon-port-pack -- import <your pack> <work>
cargo run -p horizon-port-pack -- inventory <work>
cargo run -p horizon-port-pack -- fetch <work> --from <your mods folder>
cargo run -p horizon-port-pack -- carry <work> --resources .local/horizon-client/26.3
```

Nothing it makes leaves your machine. Some tests read that content when it is in `content/` at the root of this
repository (ignored by git) and skip otherwise.

## Licences

Each mod folder has its own licence files: the original's licence for a port, MIT for the original mods. See
[LICENSE-NOTES.md](LICENSE-NOTES.md). The tools in `tools/` are MIT ([tools/LICENSE](tools/LICENSE)).

Minecraft is a trademark of Mojang Synergies AB. This repository contains no Minecraft code or assets.
