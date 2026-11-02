// Netherending's End ores in the generation (horizon_world::worldgen::generator): the original adds its
// sixteen placed features (`netherending:forge/biome_modifier/end_ores.json`, `forge:add_features`,
// step `underground_ores`) to the four outer End biomes. Horizon does not read Forge biome modifiers
// and biomes are not read from mods, so the same thing is done where the game hands out the biome
// files: the text of each outer End biome gets the features in its step 6. The game's own decoration
// then places them (count, in_square, height_range, biome filter and the vein itself, from the
// placed features the mod carries). `GeneratorData::biome_json` returns a `&str` borrowed from the
// game's table; the changed text is leaked once per distinct file (a few biomes, a few kB).
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[allow(dead_code)]
mod engine {
    include!("netherending_engine.rs");
}

static CHANGED: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

/// `GeneratorData::biome_json`'s answer, with the End ores in it for the outer End biomes.
pub(super) fn biome_json(value: &str) -> &str {
    // Only the End biomes name the enderman as their only spawn.
    if !value.contains("minecraft:enderman") {
        return value;
    }
    let mut changed = CHANGED
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(done) = changed.get(value) {
        return *done;
    }
    let Some(text) = engine::with_end_ores(value) else {
        return value;
    };
    let text: &'static str = Box::leak(text.into_boxed_str());
    changed.insert(value.to_owned(), text);
    text
}
