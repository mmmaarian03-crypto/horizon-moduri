// Ender Ore inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the
// experience the ores drop (`DropExperienceBlock(.., UniformInt.of(2, 5))`). The ores in the biomes are
// another module of this mod, in the generator (native/worldgen.rs). The blocks themselves, their loot
// (an ender pearl, Fortune, Silk Touch) and the smelting are content (content/mods/enderore).
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // Ender Ore for Horizon: the rules, without any game code.
// Written from the source of the mod (the author's own 1.20.1 rebuild, `Downloads/enderore-1201`,
// `EnderOre.java` + the `data/` of the jar, licence: all rights reserved, personal use). The mod has no
// config file in the pack's profile. This file is pasted into the game twice: as
// `crate::game::enderore::engine` (the experience of the ores) and as
// `horizon_world::worldgen::generator::enderore::engine` (the ores in the biomes); it is also the
// `horizon_enderore` crate the tests use: std + serde_json, no game types, no inner attributes.

/// The pack has no config file for this mod.
pub mod config {}

pub const MOD: &str = "enderore";

// -------------------------------------------------------------------------------- experience

/// `UniformInt.of(2, 5)` (EnderOre.XP): how much experience the three ores drop when broken with the
/// right tool and without Silk Touch (`DropExperienceBlock`).
pub const XP_MIN: i32 = 2;
pub const XP_MAX: i32 = 5;

/// The three ore blocks.
pub const BLOCKS: [&str; 3] = ["enderore:ender_ore", "enderore:deepslate_ender_ore", "enderore:end_stone_ender_ore"];

/// `RandomSource.nextInt(bound)`.
pub trait Rng {
    fn next_int_bound(&mut self, bound: i32) -> i32;
}

/// `UniformInt.sample`: `random.nextInt(max - min + 1) + min`.
pub fn sample_xp(rng: &mut impl Rng) -> i32 {
    rng.next_int_bound(XP_MAX - XP_MIN + 1) + XP_MIN
}

/// The experience a broken block drops, if it is one of the three ores (`None` for any other block,
/// whose experience the game knows).
pub fn experience(block: &str, rng: &mut impl Rng) -> Option<u32> {
    BLOCKS.contains(&block).then(|| sample_xp(rng).max(0) as u32)
}

// ------------------------------------------------------------------------------ the biomes

/// The generation step `underground_ores` (`GenerationStep.Decoration.UNDERGROUND_ORES.ordinal()`).
pub const UNDERGROUND_ORES: usize = 6;

/// `enderore:forge/biome_modifier/ender_ore.json`: `forge:add_features` of `enderore:ender_ore` to
/// `#minecraft:is_overworld`.
pub const OVERWORLD_FEATURE: &str = "enderore:ender_ore";
/// `enderore:forge/biome_modifier/end_stone_ender_ore.json`: `enderore:end_stone_ender_ore` to
/// `end_highlands`, `end_midlands`, `end_barrens` and `small_end_islands`.
pub const END_FEATURE: &str = "enderore:end_stone_ender_ore";

/// Whether a biome file (the JSON of `worldgen/biomes.json`) is one of the four outer End biomes. The
/// game gives the text without the biome's name, so the biome is told by its content: only endermen
/// spawn (`monster: [enderman]`), no rain, and not the central island (`the_end` is the one with
/// `minecraft:end_spike`). In the 26.3 data these are exactly the four.
pub fn is_outer_end_biome(biome: &serde_json::Value) -> bool {
    use serde_json::Value;
    let spawns = &biome["attributes"]["minecraft:gameplay/natural_mob_spawns"]["argument"]["spawns_by_category"];
    let only_endermen = match spawns {
        Value::Object(map) => {
            map.len() == 1
                && map.get("monster").and_then(Value::as_array).is_some_and(|list| {
                    list.len() == 1 && list[0]["type"] == "minecraft:enderman"
                })
        }
        _ => false,
    };
    let no_rain = biome["has_precipitation"] == Value::Bool(false);
    let center = biome["features"]
        .as_array()
        .is_some_and(|steps| steps.iter().any(|s| s.to_string().contains("minecraft:end_spike")));
    only_endermen && no_rain && !center
}

/// Whether a biome file is one of `#minecraft:is_overworld`. By content again: every overworld biome
/// (and no other) carries `minecraft:ore_coal_upper` in `underground_ores` (`BiomeDefaultFeatures.
/// addDefaultOres`); in the 26.3 data the 56 biomes of the tag are exactly the ones that do.
pub fn is_overworld_biome(biome: &serde_json::Value) -> bool {
    biome["features"][UNDERGROUND_ORES]
        .as_array()
        .is_some_and(|list| list.iter().any(|v| v.as_str() == Some("minecraft:ore_coal_upper")))
}

/// Appends `name` to `step` of the biome's `features` (padding the list of steps with empty ones),
/// unless it is already there. `false` when the step is not a list of names (a tag reference).
pub fn add_feature(biome: &mut serde_json::Value, step: usize, name: &str) -> bool {
    use serde_json::Value;
    let Some(obj) = biome.as_object_mut() else {
        return false;
    };
    let steps = obj.entry("features").or_insert_with(|| Value::Array(Vec::new()));
    let Some(steps) = steps.as_array_mut() else {
        return false;
    };
    while steps.len() <= step {
        steps.push(Value::Array(Vec::new()));
    }
    let Some(list) = steps[step].as_array_mut() else {
        return false;
    };
    if !list.iter().any(|v| v.as_str() == Some(name)) {
        list.push(Value::String(name.to_owned()));
    }
    true
}

/// The biome file with this mod's ore in it (the overworld ore in the overworld biomes, the End stone
/// ore in the four outer End biomes); `None` for every other biome (or a file that is not JSON).
pub fn with_ores(biome_json: &str) -> Option<String> {
    let mut biome: serde_json::Value = serde_json::from_str(biome_json).ok()?;
    let feature = if is_overworld_biome(&biome) {
        OVERWORLD_FEATURE
    } else if is_outer_end_biome(&biome) {
        END_FEATURE
    } else {
        return None;
    };
    if !add_feature(&mut biome, UNDERGROUND_ORES, feature) {
        return None;
    }
    serde_json::to_string(&biome).ok()
}

}

/// `level.random` of the game.
struct LevelRandom<'a>(&'a mut horizon_core::random::LegacyRandom);
impl engine::Rng for LevelRandom<'_> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        use horizon_core::random::RandomSource;
        self.0.next_int_bound(bound).unwrap_or(0)
    }
}

impl Game<'_> {
    /// RETURN on `block_experience(name)`: the game knows the experience of its own blocks (it answers 0 for
    /// ours). It is asked only for a player's break with the right tool and without Silk Touch
    /// (`DropExperienceBlock.spawnAfterBreak`: `silkTouch == 0`, `Block.playerDestroy` only when harvested).
    pub(super) fn enderore_experience(&mut self, name: &str, value: u32) -> u32 {
        if value != 0 {
            return value;
        }
        engine::experience(name, &mut LevelRandom(&mut self.world.random)).unwrap_or(value)
    }
}
