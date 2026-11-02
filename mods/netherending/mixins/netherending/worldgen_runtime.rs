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
    // Netherending Ores for Horizon: the rules, without any game code.
// Written from the source of the mod (the author's own 1.20.1 rebuild, `Downloads/netherending-1201`,
// `NetherendingOres.java` + the `data/` of the jar, licence MIT) and his `netherending-common.toml`.
// This file is pasted into the game twice: as `crate::game::netherending::engine` (the break event)
// and as `horizon_world::worldgen::generator::netherending::engine` (the End ores in the biomes);
// it is also the `horizon_netherending` crate the tests use: std + serde_json, no game types, no
// inner attributes (`#![...]`).
pub type Pos = [i32; 3];

/// The values of the pack's `config/netherending-common.toml` (all defaults of the mod).
#[allow(dead_code)]
pub mod config {
    /// Chance that nearby endermen get angry when you break an End ore (0..1) Range: 0.0 ~ 1.0
    pub const END_ENDERMAN_ANGER_CHANCE: f64 = 0.35; // netherending-common.toml:5 [end] endermanAngerChance
    /// Radius in which endermen hear the broken ore Range: 0 ~ 64
    pub const END_ENDERMAN_RANGE: i32 = 16; // netherending-common.toml:8 [end] endermanRange
    /// Chance that an endermite comes out of the ore Range: 0.0 ~ 1.0
    pub const END_ENDERMITE_CHANCE: f64 = 0.05; // netherending-common.toml:11 [end] endermiteChance
    /// Radius in which zombified piglins get angry when you break a Nether ore (0 = off) Range: 0 ~ 64
    pub const NETHER_PIGMEN_RANGE: i32 = 24; // netherending-common.toml:16 [nether] pigmenRange
    /// Chance that a Nether ore explodes after it is broken (without Silk Touch) Range: 0.0 ~ 1.0
    pub const NETHER_EXPLODE_CHANCE: f64 = 0.03; // netherending-common.toml:19 [nether] explodeChance
    /// The explosion's power (TNT = 4) Range: 0.0 ~ 6.0
    pub const NETHER_EXPLODE_POWER: f64 = 2.0; // netherending-common.toml:22 [nether] explodePower
}

/// `NetherendingOres.ORES`: the metals, in the order of the original.
pub const ORES: [&str; 16] = [
    "coal", "iron", "gold", "copper", "redstone", "lapis", "diamond", "emerald", "tin", "lead", "silver",
    "nickel", "aluminum", "zinc", "osmium", "uranium",
];

/// The delay of the unstable Nether ore: `level.getGameTime() + 25` (NetherendingOres.onBreak).
pub const EXPLOSION_DELAY_TICKS: i64 = 25;

// ------------------------------------------------------------------------------ the break event

/// `level.random` as the break event draws from it.
pub trait Rng {
    /// `RandomSource.nextDouble()`.
    fn next_double(&mut self) -> f64;
    /// `RandomSource.nextFloat()`.
    fn next_float(&mut self) -> f32;
}

/// The three lines the mod shows over the hotbar (`displayClientMessage(.., true)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Endermen,
    Pigmen,
    Unstable,
}
impl Message {
    /// The translation key and the English text (`assets/netherending/lang/en_us.json`).
    pub fn text(self) -> (&'static str, &'static str) {
        match self {
            Message::Endermen => ("netherending.chat.endermen", "The Endermen felt that..."),
            Message::Pigmen => ("netherending.chat.pigmen", "The pigmen heard you mining!"),
            Message::Unstable => ("netherending.chat.unstable", "The ore is unstable..."),
        }
    }
}

/// What the world does for the event. The adapter implements it on the game.
pub trait Host {
    /// The endermen whose box meets `AABB(center).inflate(range)` get the player as target
    /// (`EnderMan.setTarget`); how many there were.
    fn anger_endermen(&mut self, center: Pos, range: i32) -> usize;
    /// A fresh endermite at `at` facing `yaw`, aiming at the player (`new Endermite`, `moveTo`,
    /// `setTarget`, `addFreshEntity`).
    fn spawn_endermite(&mut self, at: [f64; 3], yaw: f32);
    /// The zombified piglins whose box meets `AABB(center).inflate(range)` get the player as target
    /// and start their persistent anger; how many there were.
    fn anger_pigmen(&mut self, center: Pos, range: i32) -> usize;
    /// `player.displayClientMessage(Component.translatable(..), true)`.
    fn notice(&mut self, message: Message);
    /// `SoundEvents.CREEPER_PRIMED`, `SoundSource.BLOCKS`, volume 1, pitch 0.8, at the block.
    fn creeper_primed(&mut self, pos: Pos);
    /// `pendingExplosions.add(new Pending(level, pos, gameTime + delay))`.
    fn schedule_explosion(&mut self, pos: Pos, delay_ticks: i64);
}

/// What the broken block is to the mod.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    /// `state.is(netherending:end_ores)`.
    EndOre,
    /// `state.is(netherending:nether_ores)`.
    NetherOre,
    /// Anything else.
    Other,
}

/// Who breaks it, and where.
#[derive(Clone, Copy, Debug)]
pub struct Breaker {
    pub creative: bool,
    pub spectator: bool,
    /// `EnchantmentHelper.getItemEnchantmentLevel(SILK_TOUCH, mainHand) > 0`.
    pub silk_touch: bool,
    /// `level.dimension() == Level.NETHER`.
    pub in_nether: bool,
}

/// `NetherendingOres.onBreak` (`BlockEvent.BreakEvent`), line by line.
pub fn on_break(host: &mut impl Host, rng: &mut impl Rng, who: Breaker, pos: Pos, ground: Ground) {
    if who.creative || who.spectator {
        return;
    }
    let silk = who.silk_touch;
    if ground == Ground::EndOre {
        let r = config::END_ENDERMAN_RANGE;
        if r > 0 && rng.next_double() < config::END_ENDERMAN_ANGER_CHANCE && host.anger_endermen(pos, r) > 0 {
            host.notice(Message::Endermen);
        }
        if !silk && rng.next_double() < config::END_ENDERMITE_CHANCE {
            let yaw = rng.next_float() * 360.0;
            host.spawn_endermite([f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5], yaw);
        }
    } else if ground == Ground::NetherOre && who.in_nether {
        let r = config::NETHER_PIGMEN_RANGE;
        if r > 0 && host.anger_pigmen(pos, r) > 0 {
            host.notice(Message::Pigmen);
        }
        if !silk && rng.next_double() < config::NETHER_EXPLODE_CHANCE {
            host.creeper_primed(pos);
            host.notice(Message::Unstable);
            host.schedule_explosion(pos, EXPLOSION_DELAY_TICKS);
        }
    }
}

/// `Level.getEntitiesOfClass(.., new AABB(pos).inflate(r))`: whether an entity box
/// (`min`, `max`) meets the box of the block `pos` grown by `r` (`AABB.intersects`, strict).
pub fn within(pos: Pos, r: i32, min: [f64; 3], max: [f64; 3]) -> bool {
    let r = f64::from(r);
    (0..3).all(|i| {
        let lo = f64::from(pos[i]) - r;
        let hi = f64::from(pos[i]) + 1.0 + r;
        lo < max[i] && hi > min[i]
    })
}

/// The unstable ores waiting to blow (`NetherendingOres.pendingExplosions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// The dimension (`minecraft:the_nether`).
    pub dim: String,
    pub pos: Pos,
    /// The game time from which it blows.
    pub at: i64,
}

/// `NetherendingOres.onServerTick` (END phase): in list order, every pending explosion whose time has
/// come is handed to `explode` (the center is the block's, `+ 0.5`) and removed. `explode` answers
/// whether the explosion could happen here (the level is the one the player is in); when it cannot,
/// the entry is dropped all the same (the original's level may be gone, the ore is already broken).
pub fn tick(pending: &mut Vec<Pending>, now: i64, mut explode: impl FnMut(&Pending) -> bool) {
    let mut i = 0;
    while i < pending.len() {
        if now >= pending[i].at {
            let p = pending.remove(i);
            explode(&p);
        } else {
            i += 1;
        }
    }
}

/// `ServerStoppedEvent`: nothing is kept.
pub fn stopped(pending: &mut Vec<Pending>) {
    pending.clear();
}

// -------------------------------------------------------------------------- the ores in the End

/// The `features` of `netherending:forge/biome_modifier/end_ores.json`, in its order.
pub fn end_features() -> Vec<String> {
    ORES.iter().map(|o| format!("netherending:end_{o}_ore")).collect()
}

/// The generation step `underground_ores` (`GenerationStep.Decoration.UNDERGROUND_ORES.ordinal()`).
pub const UNDERGROUND_ORES: usize = 6;

/// Whether a biome file (the JSON of `worldgen/biomes.json`) is one of `end_highlands`, `end_midlands`,
/// `end_barrens` and `small_end_islands`, the biomes of the original's biome modifier. The game gives
/// the text without the biome's name, so the biome is told by its content: only endermen spawn
/// (`monster: [enderman]`), no rain, and not the central island (`the_end` is the one with
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

/// Appends `names` to `step` of the biome's `features` (padding the list of steps with empty ones),
/// skipping names already there. `false` when the step is not a list of names (a tag reference).
pub fn add_features(biome: &mut serde_json::Value, step: usize, names: &[String]) -> bool {
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
    for name in names {
        if !list.iter().any(|v| v.as_str() == Some(name.as_str())) {
            list.push(Value::String(name.clone()));
        }
    }
    true
}

/// The biome file with the End ores in the four outer End biomes; `None` for every other biome (or a
/// file that is not JSON): the biome modifier of the original (`forge:add_features`, step
/// `underground_ores`).
pub fn with_end_ores(biome_json: &str) -> Option<String> {
    let mut biome: serde_json::Value = serde_json::from_str(biome_json).ok()?;
    if !is_outer_end_biome(&biome) || !add_features(&mut biome, UNDERGROUND_ORES, &end_features()) {
        return None;
    }
    serde_json::to_string(&biome).ok()
}

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
