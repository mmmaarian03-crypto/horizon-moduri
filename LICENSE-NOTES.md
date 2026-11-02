# Licence notes

Every folder under `mods/` and `mods-nc/` is its own work with its own licence files (`LICENSE`, and `COPYING` /
`COPYING.LESSER` for the GNU licences). This page explains the choices; the files in each folder are what applies.

## The rule for a port

A port is a derivative work of the original mod. A mod is here only when its **code** licence allows a derivative work
to be shared: MIT, BSD, Apache, MPL, LGPL, GPL, CC0, the Unlicense, AFL or a Creative Commons licence without
"NoDerivatives". The port then:

- keeps the original's licence (copyleft licences stay copyleft: the port is published as source under the same
  licence),
- keeps the original's copyright and licence text (the licence file is copied verbatim from the original's repository
  or jar),
- credits the original's authors and links the original in its README.

Mods under "All Rights Reserved", "NoDerivatives", "personal use", protective or other custom licences that do not
allow sharing a port, or with an unclear licence, are **not** in this repository.

## No content of the original mods

The original mods' assets often have a licence of their own (All Rights Reserved for Create, Fuzs' mods and Guard
Villagers; CC BY-NC-SA for Applied Energistics 2 and Neat). That is why nothing of the jars is here: no textures, models,
sounds, translations, recipes, loot tables or other data files, and none of them embedded in the Rust code. Players make
that content from their own copy of the jar. Numbers and rules the code of a mod computes (tiers, speeds, chances) are
part of the code that was ported. Some settings are fixed in code with the values of the 1.20.1 pack these ports were
made for; the source says where.

## Non-commercial (`mods-nc/`)

`betteradvancements` ("Don't Be a Jerk" non-commercial licence), `explorerscompass` and `naturescompass` (CC BY-NC-SA
4.0) and `patchouli` (CC BY-NC-SA 3.0) may be shared and changed under the same licence, **never for money**.
Horizon's registry does not list non-commercial mods (Horizon's docs/modpacks.md, "Licences"), so they are offered only
here.

## Points worth knowing

- `tia` (Tiny Item Animations) is left out: its author asks not to rehost it on public platforms.
- `tlc` (The Lost Castle): LGPL-3.0 as its mods.toml and its Modrinth page declare; its repository's LICENSE.txt is
  only Forge's MDK notice.
- `torchmaster`: MIT by its repository's LICENSE (branch mc/1.20.1) and its Modrinth page; the jar's mods.toml still
  says "All Rights Reserved", a template default.
- `escape_rope`: AFL-3.0 as its jar declares (its Modrinth page says MIT); the mod has no public source.
- `a_good_place`: MIT as its authors declare (gradle.properties, mods.toml, Modrinth); the repository has no LICENSE
  file, so the MIT text here names them.
- `ecran_incarcare` draws a reduction of Horizon's icon, under Horizon's MIT licence (its LICENSE includes it).

## By licence

| Licence | Mods |
|---|---|
| `AFL-3.0` | `escape_rope` |
| `BSD-3-Clause` | `mousetweaks` |
| `CC-BY-NC-SA-3.0` | `patchouli` |
| `CC-BY-NC-SA-4.0` | `explorerscompass`, `naturescompass` |
| `CC0-1.0` | `morebows` |
| `GPL-3.0-only` | `advancednetherite`, `ironchest` |
| `LGPL-2.1-only` | `additionalbanners`, `attributefix`, `enchdesc` |
| `LGPL-3.0-only` | `ae2`, `carryon`, `fallingtree`, `hats`, `itemphysic`, `tlc`, `travelerstitles`, `waterdripsound`, `yungsbridges` |
| `LGPL-3.0-or-later` | `comforts`, `curios`, `polymorph`, `smoothswapping` |
| `LicenseRef-DBaJ-NC` | `betteradvancements` |
| `MIT` | `a_good_place`, `capsule`, `clumps`, `ecran_incarcare`, `enderore`, `enderstorage`, `forja`, `hangglider`, `modonomicon`, `moreoverlays`, `neat`, `netherending`, `oreberriesreplanted`, `pornire_rapida`, `presencefootsteps`, `simplyjetpacks`, `soulshards`, `storagedrawers`, `taburi_creative`, `torchmaster` |
| `MPL-2.0` | `cavedust`, `easyanvils`, `enchantinginfuser`, `leavesbegone`, `overflowingbars`, `visualworkbench` |
