// Drip Sounds (waterdripsound) for Horizon: the rules, without any game code.
// Written from the 1.20.1 jar (`Drip Sounds-0.6.3+1.20.1-Forge.jar`, decompiled in sources/waterdripsound/dec) and
// the pack's `config/waterdripsound.json`. This file is pasted into the game as
// `crate::game::waterdripsound::engine` (tools/hnmod.py bundle) and is also the `horizon_waterdripsound` crate the tests use.

pub type Pos = [i32; 3];

/// the pack's values (`the 1.20.1 pack/config/waterdripsound.json`).
pub mod config {
    pub const ENABLED: bool = true; // waterdripsound.json enabled
    pub const VOLUME: f64 = 0.3; // waterdripsound.json volume
    pub const DRIP_CHANCE: i32 = 10; // waterdripsound.json dripChance
    pub const USE_DRIPSTONE_SOUNDS: bool = true; // waterdripsound.json useDripstoneSounds
    pub const SOUND_CATEGORY: &str = "ambient"; // waterdripsound.json soundCategory
    pub const BLOCK_ANIMATE_RANGE_MULTIPLIER: f64 = 1.0; // waterdripsound.json blockAnimateRangeMultiplier
    pub const BLOCK_ANIMATE_COUNT_MULTIPLIER: f64 = 1.0; // waterdripsound.json blockAnimateCountMultiplier
}

/// The vanilla constants the mod's `@ModifyConstant`s replace (1.20.1 `ClientLevel` / `LevelRenderer`).
pub mod vanilla {
    /// `ClientLevel.doAnimateTick`: `random.nextInt(10) == 0` before a fluid drips.
    pub const DRIP_CHANCE: i32 = 10;
    /// `ClientLevel.animateTick`: 667 blocks animated per tick, in ranges 16 and 32.
    pub const ANIMATE_COUNT: i32 = 667;
    pub const ANIMATE_RANGE_1: i32 = 16;
    pub const ANIMATE_RANGE_2: i32 = 32;
    /// `LevelRenderer.addParticleInternal`: particles further than this squared distance are dropped.
    pub const PARTICLE_ADD_RANGE: f64 = 1024.0;
}

/// `MixinClientLevel.modifyAnimateCount/Range*`: `(int)((double)original * multiplier)`.
pub fn scaled(original: i32, multiplier: f64) -> i32 {
    (f64::from(original) * multiplier) as i32
}

/// `MixinLevelRenderer.modifyMaxParticleAddRange`: `Math.max(original, original * multiplier)`.
pub fn add_range(multiplier: f64) -> f64 {
    vanilla::PARTICLE_ADD_RANGE.max(vanilla::PARTICLE_ADD_RANGE * multiplier)
}

/// `SoundEvents.POINTED_DRIPSTONE_DRIP_WATER` / `_DRIP_LAVA` (SRG `f_144128_` / `f_144127_`, read off the 1.20.1
/// mappings: `gr` / `gq` in `client-1.20.1-...-mappings.txt`).
pub const DRIP_WATER: &str = "minecraft:block.pointed_dripstone.drip_water";
pub const DRIP_LAVA: &str = "minecraft:block.pointed_dripstone.drip_lava";

/// 1.20.1 `SoundSource` in declaration order (`getName()`), the choices of the config screen.
/// Horizon's `SOUND_SOURCES` has the same order with `ui` added at the end.
pub const SOURCES: [&str; 10] = [
    "master", "music", "record", "weather", "block", "hostile", "neutral", "player", "ambient", "voice",
];

/// The drip particle that landed (`DripParticle.type`): the non-dripstone `FallAndLandParticle`s are the falling
/// water (`Fluids.WATER`) and falling lava (`Fluids.LAVA`) drips; honey and obsidian tears have `Fluids.EMPTY`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drip {
    Water,
    Lava,
}

/// `FluidState.getType()` of the block under the landing point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fluid {
    Empty,
    Water,
    FlowingWater,
    Lava,
    FlowingLava,
}
impl Fluid {
    /// A block's fluid from its facts: a source is level 8 and not falling (`FluidState.isSource`).
    pub fn of(water: bool, lava: bool, level: u8, falling: bool) -> Fluid {
        let source = level == 8 && !falling;
        match (water, lava, source) {
            (true, _, true) => Fluid::Water,
            (true, _, false) => Fluid::FlowingWater,
            (_, true, true) => Fluid::Lava,
            (_, true, false) => Fluid::FlowingLava,
            _ => Fluid::Empty,
        }
    }
}

/// The player's choices (the Cloth screen of the original; the pack's file by default).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// 0.0 ..= 1.0 (`volume`; the screen shows it in percent).
    pub volume: f64,
    pub use_dripstone_sounds: bool,
    /// A name of `SOURCES`.
    pub category: String,
}
impl Settings {
    pub fn tuned() -> Self {
        Settings {
            enabled: config::ENABLED,
            volume: config::VOLUME,
            use_dripstone_sounds: config::USE_DRIPSTONE_SOUNDS,
            category: config::SOUND_CATEGORY.to_owned(),
        }
    }
    /// The volume as the screen's slider shows it: `(int)(volume * 100)`.
    pub fn volume_percent(&self) -> i32 {
        (self.volume * 100.0) as i32
    }
    /// `readMainConfig`: an unknown category falls back to AMBIENT.
    pub fn category_index(&self) -> usize {
        SOURCES.iter().position(|s| *s == self.category).unwrap_or(8)
    }
}

/// The slider in steps of 10 (`startIntSlider(.., 0, 100)`), past 100 back to 0 (muted).
pub fn next_volume_percent(p: i32) -> i32 {
    if p >= 100 { 0 } else { ((p / 10 + 1) * 10).min(100) }
}

/// The enum selector: the next `SoundSource`.
pub fn next_category(name: &str) -> &'static str {
    let i = SOURCES.iter().position(|s| *s == name).unwrap_or(8);
    SOURCES[(i + 1) % SOURCES.len()]
}

/// `MixinFallAndLandParticle.onLand`: the block it looks at, `new BlockPos((int)x, (int)y - 1, (int)z)` (Java's
/// `(int)` truncates towards zero, so this is not the block under the drip at negative coordinates).
pub fn below(at: [f64; 3]) -> Pos {
    [at[0] as i32, (at[1] as i32).wrapping_sub(1), at[2] as i32]
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub event: &'static str,
    pub volume: f32,
    pub pitch: f32,
}

/// `MixinFallAndLandParticle.onLand`: the sound of a drip that just hit the ground. `random` is `Math.random()`.
pub fn land_sound(s: &Settings, drip: Drip, below: Fluid, random: f64) -> Option<Sound> {
    if !s.enabled {
        return None;
    }
    let event = match drip {
        // particleFluid == LAVA && belowFluid.getType() != LAVA (flowing lava is FLOWING_LAVA)
        Drip::Lava if below != Fluid::Lava => DRIP_LAVA,
        // particleFluid == WATER && belowFluid.isEmpty()
        Drip::Water if below == Fluid::Empty => DRIP_WATER,
        _ => return None,
    };
    let volume_mod = 1.0f32;
    // Mth.clamp(GENERAL.volume.floatValue() * volumeMod, 0, 1)
    let mut vol = ((s.volume as f32) * volume_mod).clamp(0.0, 1.0);
    if s.use_dripstone_sounds {
        vol = (f64::from(vol) * (random * 0.7 + 0.3)) as f32;
    }
    Some(Sound { event, volume: vol, pitch: 1.0 })
}
