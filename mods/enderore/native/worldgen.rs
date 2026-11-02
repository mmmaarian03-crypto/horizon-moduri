// Ender Ore's ores in the generation (horizon_world::worldgen::generator): the original adds
// `enderore:ender_ore` to `#minecraft:is_overworld` and `enderore:end_stone_ender_ore` to the four outer
// End biomes (`forge/biome_modifier/*.json`, `forge:add_features`, step `underground_ores`). Horizon does
// not read Forge biome modifiers and biomes are not read from mods, so the same thing is done where the
// game hands out the biome files: the text of each such biome gets the feature in its step 6. The game's
// own decoration then places it (count, in_square, height_range, biome filter and the vein itself, from
// the placed features the mod carries). `GeneratorData::biome_json` returns a `&str` borrowed from the
// game's table; the changed text is leaked once per distinct file (about sixty biomes, a few kB each).
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[allow(dead_code)]
mod engine {
    include!("enderore_engine.rs");
}

static CHANGED: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

/// `GeneratorData::biome_json`'s answer, with the ore in it for the biomes of the original's modifiers.
pub(super) fn biome_json(value: &str) -> &str {
    // Both kinds of biome name an ore-bearing or enderman-only list; the cheap test first.
    if !value.contains("minecraft:ore_coal_upper") && !value.contains("minecraft:enderman") {
        return value;
    }
    let mut changed = CHANGED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(done) = changed.get(value) {
        return *done;
    }
    let Some(text) = engine::with_ores(value) else {
        return value;
    };
    let text: &'static str = Box::leak(text.into_boxed_str());
    changed.insert(value.to_owned(), text);
    text
}
