// Cave Dust inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (src/lib.rs); this part reads the world and the player once a tick and adds the particles.
//
// Cave Dust is a client mod: it runs on the game that has the window (`native_window`), once per
// client tick (the original listens to Architectury's `ClientTickEvent.CLIENT_POST`), and not on a
// server's own game.
use super::{Dim, Game};
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_core::random::{LegacyRandom, RandomSource};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    // Cave Dust for Horizon: the rules of the dust, without any game code.
// Written from the behaviour of Cave Dust 2.0.4 for Forge 1.20.1 (LizIsTired, MPL-2.0), read from
// its compiled classes (`CaveDustImpl.createCaveDust`, `ParticleSpawnUtilImpl.shouldParticlesSpawn`,
// `ConfigForge`, `CaveDustConfigScreen.getParticle`) and from `config/cavedust.toml` of the pack;
// no code of the original is used. The vanilla 1.20.1 particle order comes from the client jar.

/// A block position.
pub type Pos = [i32; 3];

/// `Level.getSeaLevel()` of a `ClientLevel` in 1.20.1 (the constant 63).
pub const SEA_LEVEL: i32 = 63;
/// The biome in which nothing spawns.
pub const LUSH_CAVES: &str = "minecraft:lush_caves";

/// The options of `cavedust.toml` this port follows. The three dimension options of the original
/// are registered under the one key `dimensionX`, so they always hold the same value.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// `dimensionX` (and `Y`, `Z`): 1 to 50, how far from the player dust is sought.
    pub dimension: i32,
    /// `caveDustEnabled`.
    pub enabled: bool,
    /// `seaLevelCheck`: defined by the original but never read by its code.
    pub sea_level_check: bool,
    /// `superFlatStatus`.
    pub superflat_status: bool,
    /// `upperLimit` (0 to 255): at and above this height no dust.
    pub upper_limit: i32,
    /// `lowerLimit` (-255 to 0): at and below this height the most dust.
    pub lower_limit: i32,
    /// `particleMultiplier` (1 to 100).
    pub particle_multiplier: i32,
    /// `particleMultiplierMultiplier` (1 to 100).
    pub particle_multiplier_multiplier: i32,
    /// `particleID`: the index of the particle in the 1.20.1 particle registry.
    pub particle_id: i32,
}
impl Default for Settings {
    /// the pack's `config/cavedust.toml`.
    fn default() -> Self {
        Self {
            dimension: 5,
            enabled: true,
            sea_level_check: true,
            superflat_status: false,
            upper_limit: 64,
            lower_limit: -64,
            particle_multiplier: 1,
            particle_multiplier_multiplier: 10,
            particle_id: 80,
        }
    }
}

/// The particles of the 1.20.1 registry in registry order (`BuiltInRegistries.PARTICLE_TYPE`):
/// `particleID` is an index into this list.
pub const PARTICLES: [&str; 95] = [
    "ambient_entity_effect",
    "angry_villager",
    "block",
    "block_marker",
    "bubble",
    "cloud",
    "crit",
    "damage_indicator",
    "dragon_breath",
    "dripping_lava",
    "falling_lava",
    "landing_lava",
    "dripping_water",
    "falling_water",
    "dust",
    "dust_color_transition",
    "effect",
    "elder_guardian",
    "enchanted_hit",
    "enchant",
    "end_rod",
    "entity_effect",
    "explosion_emitter",
    "explosion",
    "sonic_boom",
    "falling_dust",
    "firework",
    "fishing",
    "flame",
    "cherry_leaves",
    "sculk_soul",
    "sculk_charge",
    "sculk_charge_pop",
    "soul_fire_flame",
    "soul",
    "flash",
    "happy_villager",
    "composter",
    "heart",
    "instant_effect",
    "item",
    "vibration",
    "item_slime",
    "item_snowball",
    "large_smoke",
    "lava",
    "mycelium",
    "note",
    "poof",
    "portal",
    "rain",
    "smoke",
    "sneeze",
    "spit",
    "squid_ink",
    "sweep_attack",
    "totem_of_undying",
    "underwater",
    "splash",
    "witch",
    "bubble_pop",
    "current_down",
    "bubble_column_up",
    "nautilus",
    "dolphin",
    "campfire_cosy_smoke",
    "campfire_signal_smoke",
    "dripping_honey",
    "falling_honey",
    "landing_honey",
    "falling_nectar",
    "falling_spore_blossom",
    "ash",
    "crimson_spore",
    "warped_spore",
    "spore_blossom_air",
    "dripping_obsidian_tear",
    "falling_obsidian_tear",
    "landing_obsidian_tear",
    "reverse_portal",
    "white_ash",
    "small_flame",
    "snowflake",
    "dripping_dripstone_lava",
    "falling_dripstone_lava",
    "dripping_dripstone_water",
    "falling_dripstone_water",
    "glow_squid_ink",
    "glow",
    "wax_on",
    "wax_off",
    "electric_spark",
    "scrape",
    "shriek",
    "egg_crack",
];
/// The particle types that are not `SimpleParticleType`s: casting them to `ParticleOptions` fails
/// in the original, which then falls back to white ash.
pub const OPTION_PARTICLES: [&str; 9] = [
    "block",
    "block_marker",
    "dust",
    "dust_color_transition",
    "falling_dust",
    "sculk_charge",
    "item",
    "vibration",
    "shriek",
];
/// The particle the original falls back to (`ParticleTypes.WHITE_ASH`).
pub const WHITE_ASH: &str = "white_ash";

/// `CaveDustConfigScreen.getParticle`: the particle `particleID` names, white ash when it is out of
/// the registry or needs options (`ClassCastException` in the original).
pub fn particle_name(id: i32) -> &'static str {
    match usize::try_from(id).ok().and_then(|i| PARTICLES.get(i)) {
        Some(name) if !OPTION_PARTICLES.contains(name) => name,
        _ => WHITE_ASH,
    }
}

/// `MathHelper.normalize`: 1 at `min`, 0 at `max`, outside that range beyond 1 or below 0.
pub fn normalize(min: f64, max: f64, value: f64) -> f64 {
    1.0 - ((value - min) / (max - min))
}

/// `PARTICLE_AMOUNT`: how many places are tried each tick at block height `block_y`. The product is
/// cut toward zero as Java's `(int)` does, so above the upper limit it is negative (no tries).
pub fn particle_amount(s: &Settings, block_y: i32) -> i32 {
    (normalize(
        f64::from(s.lower_limit),
        f64::from(s.upper_limit),
        f64::from(block_y),
    ) * f64::from(s.particle_multiplier)
        * f64::from(s.particle_multiplier_multiplier)) as i32
}

/// A source of numbers in `[0, 1)`.
pub trait Rng {
    fn next_f64(&mut self) -> f64;
}

/// `ThreadLocalRandom.nextDouble(origin, bound)`.
pub fn random_double(rng: &mut impl Rng, origin: f64, bound: f64) -> f64 {
    if origin >= bound {
        return 0.0;
    }
    let r = rng.next_f64() * (bound - origin) + origin;
    if r >= bound { bound.next_down() } else { r }
}

/// One coordinate of a try: the player's coordinate plus a whole offset of up to `dimension`
/// either way, both cut toward zero like `(int) (pos + (int) random)`.
pub fn pick(rng: &mut impl Rng, from: f64, dimension: i32) -> i32 {
    let reach = f64::from(dimension);
    let offset = random_double(rng, -reach, reach) as i32;
    (from + f64::from(offset)) as i32
}

/// What the dust reads of the world.
pub trait Level {
    /// `Minecraft.isPaused()`.
    fn paused(&self) -> bool;
    /// The dimension type's `bedWorks` (only the overworld among the vanilla dimensions).
    fn bed_works(&self) -> bool;
    /// `Level.getMinBuildHeight()`.
    fn bottom_y(&self) -> i32;
    /// Whether the biome at `pos` is lush caves.
    fn lush_caves(&self, pos: Pos) -> bool;
    /// `ClientLevelData.isFlat()`.
    fn flat_world(&self) -> bool;
    /// `Level.canSeeSky(pos)`: the sky light there is 15.
    fn sky_visible(&self, pos: Pos) -> bool;
    /// `BlockState.isAir()`.
    fn is_air(&self, pos: Pos) -> bool;
}

/// Tries in the dark the original wants before its first particle (`timer > 10`).
pub const WARM_UP: f32 = 10.0;

/// The one thing the original remembers between tries: a counter that grows by one at every try
/// in a dark place and is cleared by every try a general rule refuses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dust {
    pub timer: f32,
}
impl Dust {
    /// `ParticleSpawnUtilImpl.shouldParticlesSpawn(client, pos)`.
    pub fn should_spawn(&mut self, s: &Settings, level: &impl Level, pos: Pos) -> bool {
        if !s.enabled
            || level.paused()
            || !level.bed_works()
            || level.bottom_y() > pos[1]
            || level.lush_caves(pos)
        {
            self.timer = 0.0;
            return false;
        }
        // The original asks for a flat world only when `superFlatStatus` is on.
        if s.superflat_status && level.flat_world() {
            return false;
        }
        if !level.sky_visible(pos) && pos[1] + 2 < SEA_LEVEL {
            self.timer += 1.0;
            if self.timer > WARM_UP {
                self.timer = WARM_UP;
                return true;
            }
        }
        false
    }

    /// `CaveDustImpl.createCaveDust` for one tick: the places where a particle is to be added.
    /// `feet` is the player's position, whose block height sets how many places are tried.
    pub fn tick(
        &mut self,
        s: &Settings,
        level: &impl Level,
        rng: &mut impl Rng,
        feet: [f64; 3],
    ) -> Vec<Pos> {
        let block_y = feet[1].floor() as i32;
        let tries = particle_amount(s, block_y);
        let mut out = Vec::new();
        for _ in 0..tries.max(0) {
            let x = pick(rng, feet[0], s.dimension);
            let y = pick(rng, feet[1], s.dimension);
            let z = pick(rng, feet[2], s.dimension);
            let pos = [x, y, z];
            if self.should_spawn(s, level, pos) && level.is_air(pos) {
                out.push(pos);
            }
        }
        out
    }
}

}

/// `ThreadLocalRandom` stand-in: the game's own random generator type, seeded from the clock, so
/// the dust never touches the world's or the particle engine's random sequences.
impl engine::Rng for LegacyRandom {
    fn next_f64(&mut self) -> f64 {
        self.next_double()
    }
}

/// What the mod remembers: the original's static `timer` and a random generator.
struct Memory {
    dust: engine::Dust,
    rng: LegacyRandom,
}
thread_local! {
    static MEMORY: RefCell<Memory> = RefCell::new(Memory {
        dust: engine::Dust::default(),
        rng: LegacyRandom::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x5DEE_CE66, |d| d.as_nanos() as i64),
        ),
    });
}

/// The world as the dust sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::Level for Live<'_, '_> {
    fn paused(&self) -> bool {
        self.game.pause
    }
    /// Only the overworld among the vanilla dimension types has `bed_works`.
    fn bed_works(&self) -> bool {
        self.game.dim == Dim::Overworld
    }
    fn bottom_y(&self) -> i32 {
        self.game.dim.min_y()
    }
    fn lush_caves(&self, p: engine::Pos) -> bool {
        let g = self.game;
        g.world
            .chunks
            .get(&(p[0] >> 4, p[2] >> 4))
            .is_some_and(|chunk| {
                g.res.biomes().get(chunk.biome(p[0], p[1], p[2])).name == engine::LUSH_CAVES
            })
    }
    /// Horizon has no flat-world flag on the client; with `superFlatStatus` off (the pack's
    /// setting) the original never asks anyway.
    fn flat_world(&self) -> bool {
        false
    }
    /// `canSeeSky`: sky light 15 (the high half of the packed light).
    fn sky_visible(&self, p: engine::Pos) -> bool {
        self.game.world.light(p[0], p[1], p[2]) >> 4 == 15
    }
    fn is_air(&self, p: engine::Pos) -> bool {
        let g = self.game;
        if p[1] < g.dim.min_y() || p[1] > g.dim.max_y() {
            return false;
        }
        let state = g.world.block(p[0], p[1], p[2]);
        matches!(
            &*g.res.facts().get(state).name,
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
}

impl Game<'_> {
    /// One client tick of Cave Dust: dust in the dark air around a player who is underground.
    pub(super) fn cavedust_tick(&mut self) {
        // The window's game only, in a world, not behind the title screen.
        if !self.native_window() || self.title.is_some() {
            return;
        }
        let Some(feet) = self
            .player
            .as_ref()
            .map(|p| [p.position.x, p.position.y, p.position.z])
        else {
            return;
        };
        let settings = engine::Settings::default();
        let places = MEMORY.with(|m| {
            let m = &mut *m.borrow_mut();
            m.dust
                .tick(&settings, &Live { game: &*self }, &mut m.rng, feet)
        });
        if places.is_empty() {
            return;
        }
        // `particleID`, falling back to white ash when it names nothing the engine can make.
        let ty = ParticleType::from_name(engine::particle_name(settings.particle_id))
            .unwrap_or(ParticleType::WhiteAsh);
        let options = ParticleOptions::simple(ty);
        for p in places {
            // `addParticle(particle, x, y, z, 0, 0, 0)` at the corner of the block.
            self.particles
                .add(&options, p.map(f64::from), [0.0, 0.0, 0.0]);
        }
    }
}
