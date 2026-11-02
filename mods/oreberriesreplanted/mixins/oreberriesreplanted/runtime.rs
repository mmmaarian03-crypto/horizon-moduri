// Oreberries Replanted inside Horizon's game (nightly 142388e1c136). The rules are the
// engine below; this part reads the world the game's way: the bushes' random ticks
// (OreBerryBushBlock.tick), what a click does (use, attack), what a bush does to whoever
// stands in it (entityInside), where a bush may be placed (canSurvive) and the essence
// berry (EssenceBerryItem.use). The bushes are plain blocks here: see content.rs.
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // Oreberries Replanted for Horizon: the rules, without any game code.
// Written from the 1.20.1 source of the mod (Mrbysco/OreBerries-Replanted, branch 1.20,
// version 0.5.2, MIT) and the pack's `config/oreberriesreplanted-common.toml`. The numbers
// are quoted with the Java class they come from.

pub const MOD: &str = "oreberriesreplanted";
pub type Pos = [i32; 3];

/// The twelve bushes (`OreEnum`), in the enum's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ore {
    Iron,
    Gold,
    Copper,
    Tin,
    Aluminum,
    Lead,
    Nickel,
    Uranium,
    Osmium,
    Zinc,
    Silver,
    Essence,
}
pub const ORES: [Ore; 12] = [
    Ore::Iron,
    Ore::Gold,
    Ore::Copper,
    Ore::Tin,
    Ore::Aluminum,
    Ore::Lead,
    Ore::Nickel,
    Ore::Uranium,
    Ore::Osmium,
    Ore::Zinc,
    Ore::Silver,
    Ore::Essence,
];
impl Ore {
    pub fn index(self) -> usize {
        ORES.iter().position(|o| *o == self).unwrap_or(0)
    }
    pub fn name(self) -> &'static str {
        match self {
            Ore::Iron => "iron",
            Ore::Gold => "gold",
            Ore::Copper => "copper",
            Ore::Tin => "tin",
            Ore::Aluminum => "aluminum",
            Ore::Lead => "lead",
            Ore::Nickel => "nickel",
            Ore::Uranium => "uranium",
            Ore::Osmium => "osmium",
            Ore::Zinc => "zinc",
            Ore::Silver => "silver",
            Ore::Essence => "essence",
        }
    }
    /// `iron_oreberry_bush`, but `essence_berry_bush`.
    pub fn bush(self) -> String {
        match self {
            Ore::Essence => format!("{MOD}:essence_berry_bush"),
            o => format!("{MOD}:{}_oreberry_bush", o.name()),
        }
    }
    /// `iron_oreberry`, but `essence_berry`: what the bush gives.
    pub fn berry(self) -> String {
        match self {
            Ore::Essence => format!("{MOD}:essence_berry"),
            o => format!("{MOD}:{}_oreberry", o.name()),
        }
    }
    pub fn from_bush(id: &str) -> Option<Ore> {
        ORES.into_iter().find(|o| o.bush() == id)
    }
}

/// `OreBerriesConfig.COMMON` with the values of the pack's config file (all defaults).
#[derive(Clone, Debug)]
pub struct Settings {
    /// `growthChance`: the 1 in X chance a bush grows when it is ticked.
    pub growth_chance: i32,
    /// `darknessOnly<Ore>Bush`, in `ORES` order.
    pub darkness_only: [bool; 12],
    /// `<ore>BushDensity`, in `ORES` order.
    pub density: [i32; 12],
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            growth_chance: 20,
            darkness_only: [true; 12],
            // iron 1, gold 1, copper 2, tin 2, aluminum 2, lead 1, nickel 1, uranium 1,
            // osmium 1, zinc 2, silver 1, essence 2.
            density: [1, 1, 2, 2, 2, 1, 1, 1, 1, 2, 1, 2],
        }
    }
}
impl Settings {
    pub fn dark_only(&self, ore: Ore) -> bool {
        self.darkness_only[ore.index()]
    }
    pub fn density(&self, ore: Ore) -> i32 {
        self.density[ore.index()]
    }
}

/// Random draws the way the world gives them.
pub trait Rng {
    fn next_int_bound(&mut self, bound: i32) -> i32;
    fn next_boolean(&mut self) -> bool;
}

/// A small generator for the parts that need no particular sequence (the random
/// ticks, the seeds of the world generation).
#[derive(Clone, Debug)]
pub struct Prng(pub u64);
impl Prng {
    /// A generator for one chunk and one feature: the same chunk always gets the same bushes.
    pub fn for_feature(chunk_x: i32, chunk_z: i32, ore: Ore) -> Self {
        let mut s = (u64::from(chunk_x as u32) << 32) ^ u64::from(chunk_z as u32);
        s = s.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (0xB0B3 + ore.index() as u64);
        let mut p = Prng(s);
        // Warm up: splitmix64's first outputs correlate with close seeds.
        for _ in 0..4 {
            p.next_u64();
        }
        p
    }
    pub fn next_u64(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
impl Rng for Prng {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        if bound <= 1 {
            return 0;
        }
        ((self.next_u64() >> 33) % bound as u64) as i32
    }
    fn next_boolean(&mut self) -> bool {
        self.next_u64() >> 63 == 1
    }
}

// ------------------------------------------------------------------- the bush

pub const MAX_AGE: i32 = 3;

/// `Level.getRawBrightness(pos, 0)`: the brighter of the sky and block light.
pub fn raw_brightness(sky: u8, block: u8) -> i32 {
    i32::from(sky.max(block))
}

/// `OreBerryBushBlock.tick`: the new age, if the bush grows. The 1 in
/// `growthChance` draw is made whenever the bush is not at its last age (it is the
/// first operand of the original's condition); the dark check follows.
pub fn grow(s: &Settings, ore: Ore, age: i32, brightness: i32, rng: &mut impl Rng) -> Option<i32> {
    if age >= MAX_AGE {
        return None;
    }
    if rng.next_int_bound(s.growth_chance.max(1)) != 0 {
        return None;
    }
    // flag = !darknessOnly || getRawBrightness(pos, 0) < 10
    if !s.dark_only(ore) || brightness < 10 {
        Some(age + 1)
    } else {
        None
    }
}

/// What `harvestBerry` does to a grown bush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Harvest {
    /// `withAge(getMaxAge() - 1)`.
    pub new_age: i32,
    /// `level.random.nextInt(3) + 1` berries.
    pub berries: i32,
}
/// `OreBerryBushBlock.harvestBerry` (a right click and the start of a left click).
pub fn harvest(age: i32, rng: &mut impl Rng) -> Option<Harvest> {
    if age < MAX_AGE {
        return None;
    }
    Some(Harvest {
        new_age: MAX_AGE - 1,
        berries: rng.next_int_bound(3) + 1,
    })
}

/// What stands under a bush.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// An ore berry bush: `canSustainPlant` says yes from the second age on.
    Bush { age: i32 },
    /// Any other block: Forge's `PlantType.CAVE` wants its top face sturdy.
    Block { sturdy_top: bool },
}
/// `OreBerryBushBlock.canSurvive`, asked when the bush is placed (the block is a plain
/// `Block`: it does not ask again when a neighbour changes).
pub fn can_place(s: &Settings, ore: Ore, brightness: i32, below: Support) -> bool {
    // flag = !darknessOnly || getRawBrightness(pos, 0) < 13
    let dark = !s.dark_only(ore) || brightness < 13;
    let supported = match below {
        Support::Bush { age } => age >= 2,
        Support::Block { sturdy_top } => sturdy_top,
    };
    dark && supported
}

/// The outline and collision of each age (`SHAPE_BY_AGE`), in blocks: min then max.
pub fn shape(age: i32) -> [f64; 6] {
    match age {
        0 => [4.0 / 16.0, 0.0, 4.0 / 16.0, 12.0 / 16.0, 8.0 / 16.0, 12.0 / 16.0],
        1 => [2.0 / 16.0, 0.0, 2.0 / 16.0, 14.0 / 16.0, 12.0 / 16.0, 14.0 / 16.0],
        _ => [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    }
}

/// The vanilla block whose collision a bush of this age takes (the game has no way to add a
/// new shape to its tables here, so each age points at an existing one of about its size).
/// `SHAPE_BY_AGE` is a box of 8x8x8 (age 0), 12x12x12 (age 1), then the whole block.
pub fn collision_like(age: i32) -> &'static str {
    match age {
        0 => "minecraft:turtle_egg",
        1 => "minecraft:chest",
        _ => "minecraft:stone",
    }
}

/// `entityInside`: every entity but an item takes 1 point of cactus damage.
pub const HURT_SOURCE: &str = "minecraft:cactus";
pub const HURT_AMOUNT: f32 = 1.0;

/// The blocks an entity's box touches (`Entity.checkInsideBlocks`: the box's corners moved
/// 1e-7 inwards).
pub fn cells_touched(min: [f64; 3], max: [f64; 3]) -> Vec<Pos> {
    let lo = [0, 1, 2].map(|i| (min[i] + 1.0e-7).floor() as i32);
    let hi = [0, 1, 2].map(|i| (max[i] - 1.0e-7).floor() as i32);
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                out.push([x, y, z]);
            }
        }
    }
    out
}

// -------------------------------------------------------------- the inert chain

/// The Java classes a bush or a vat copies from its template (the sweet berry bush, the
/// cauldron) that Horizon would give behaviour: survival on dirt, bone meal, the
/// vanilla berries, filling with water. The mod's own blocks are plain `Block`s, and
/// their behaviour is written in this mod.
pub fn inert_class(java_class: &str) -> Option<&'static str> {
    match java_class {
        "SweetBerryBushBlock" | "BushBlock" | "CauldronBlock" | "AbstractCauldronBlock" => Some("Block"),
        "BonemealableBlock" => Some("Inert"),
        _ => None,
    }
}

// -------------------------------------------------------------- world generation

/// One block as the generation reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Air,
    CaveAir,
    /// `#minecraft:base_stone_overworld`.
    BaseStone,
    Other,
}
impl Cell {
    /// `Level.isEmptyBlock`.
    pub fn is_empty(self) -> bool {
        matches!(self, Cell::Air | Cell::CaveAir)
    }
}
/// The blocks of `#minecraft:base_stone_overworld`.
pub fn is_base_stone(name: &str) -> bool {
    matches!(
        name,
        "minecraft:stone"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:tuff"
            | "minecraft:deepslate"
    )
}

pub trait GenWorld {
    /// The block at `p`; positions outside the area being decorated and outside the
    /// build height read as `Air` (void air) and cannot be written.
    fn cell(&self, p: Pos) -> Cell;
    /// `setBlock(pos, bush[age=3], 2)`.
    fn set_bush(&mut self, p: Pos, ore: Ore);
}

/// `OreBerryFeatures` (`chance`, the node size) and `OreBerryPlacements`
/// (`ChanceRangePlacement(bottom, 0, maximum, rarity)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feature {
    pub chance: i32,
    pub bottom_offset: i32,
    pub top_offset: i32,
    pub maximum: i32,
    pub rarity: i32,
}
pub fn feature(ore: Ore) -> Feature {
    let (chance, bottom, maximum, rarity) = match ore {
        Ore::Iron => (12, -64, 64, 5),
        Ore::Gold => (6, -64, 32, 8),
        Ore::Copper => (12, -44, 60, 3),
        Ore::Tin => (12, -44, 60, 3),
        Ore::Aluminum => (14, -44, 60, 2),
        Ore::Lead => (14, -64, 40, 7),
        Ore::Nickel => (12, -64, 120, 5),
        Ore::Uranium => (14, -64, 60, 5),
        Ore::Osmium => (13, -64, 60, 6),
        Ore::Zinc => (12, -64, 70, 6),
        Ore::Silver => (14, -64, 40, 7),
        Ore::Essence => (8, -64, 32, 6),
    };
    Feature {
        chance,
        bottom_offset: bottom,
        top_offset: 0,
        maximum,
        rarity,
    }
}

fn add(p: Pos, d: [i32; 3]) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}
const UP: [i32; 3] = [0, 1, 0];
const DOWN: [i32; 3] = [0, -1, 0];
const EAST: [i32; 3] = [1, 0, 0];
const WEST: [i32; 3] = [-1, 0, 0];
const SOUTH: [i32; 3] = [0, 0, 1];
const NORTH: [i32; 3] = [0, 0, -1];

/// `OreBerryBushFeature.findAdequateLocation`: an empty block with a solid one above it
/// (the solid one is returned), searched upwards 16 blocks, then an empty block with a
/// solid one under it (that one), downwards 16 blocks.
pub fn find_adequate_location(world: &impl GenWorld, origin: Pos) -> Option<Pos> {
    let mut pos = origin;
    for _ in 0..16 {
        if world.cell(pos).is_empty() && !world.cell(add(pos, UP)).is_empty() {
            return Some(add(pos, UP));
        }
        pos = add(pos, UP);
    }
    let mut pos = origin;
    for _ in 0..16 {
        if world.cell(pos).is_empty() && !world.cell(add(pos, DOWN)).is_empty() {
            return Some(add(pos, DOWN));
        }
        pos = add(pos, DOWN);
    }
    None
}

/// `generateBerryBlock`: a bush where a block touches air above or below it and is cave air
/// or base stone.
fn berry_block(world: &mut impl GenWorld, pos: Pos, ore: Ore) {
    let here = world.cell(pos);
    if (!world.cell(add(pos, UP)).is_empty() || !world.cell(add(pos, DOWN)).is_empty())
        && (here == Cell::CaveAir || here == Cell::BaseStone)
    {
        world.set_bush(pos, ore);
    }
}
fn tiny_node(world: &mut impl GenWorld, rng: &mut impl Rng, pos: Pos, ore: Ore) {
    berry_block(world, pos, ore);
    for d in [EAST, WEST, SOUTH, NORTH] {
        if rng.next_int_bound(4) == 0 {
            berry_block(world, add(pos, d), ore);
        }
    }
    if rng.next_int_bound(16) < 7 {
        berry_block(world, add(pos, UP), ore);
    }
}
fn small_node(world: &mut impl GenWorld, rng: &mut impl Rng, pos: Pos, ore: Ore) {
    berry_block(world, pos, ore);
    for d in [EAST, WEST, SOUTH, NORTH] {
        if rng.next_boolean() {
            berry_block(world, add(pos, d), ore);
        }
    }
    if rng.next_int_bound(4) != 0 {
        berry_block(world, add(pos, UP), ore);
    }
}
fn medium_node(world: &mut impl GenWorld, rng: &mut impl Rng, pos: Pos, ore: Ore) {
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if rng.next_int_bound(4) == 0 {
                    berry_block(world, add(pos, [x, y, z]), ore);
                }
            }
        }
    }
    small_node(world, rng, pos, ore);
}

/// `OreBerryBushFeature.place` at `origin`: `density` tries, each a node of the size the
/// `chance` draw says.
pub fn place_feature(
    world: &mut impl GenWorld,
    rng: &mut impl Rng,
    s: &Settings,
    ore: Ore,
    origin: Pos,
) {
    let f = feature(ore);
    for _ in 0..s.density(ore) {
        if let Some(at) = find_adequate_location(world, origin) {
            let kind = rng.next_int_bound(f.chance);
            if kind == 11 {
                medium_node(world, rng, at, ore);
            } else if kind >= 5 {
                small_node(world, rng, at, ore);
            } else {
                tiny_node(world, rng, at, ore);
            }
        }
    }
}

/// `ChanceRangePlacement` then `InSquarePlacement`: the feature's origin in the chunk
/// at (`chunk_x`, `chunk_z`), or none (one chunk in `rarity` has the feature at all).
pub fn feature_origin(rng: &mut impl Rng, ore: Ore, chunk_x: i32, chunk_z: i32) -> Option<Pos> {
    let f = feature(ore);
    if rng.next_int_bound(f.rarity) != 0 {
        return None;
    }
    let y = rng.next_int_bound(f.maximum - f.top_offset) + f.bottom_offset;
    let x = chunk_x * 16 + rng.next_int_bound(16);
    let z = chunk_z * 16 + rng.next_int_bound(16);
    Some([x, y, z])
}

/// The `underground_ores` step of a freshly generated overworld chunk: each bush's
/// placed feature in turn, each with its own generator.
pub fn decorate_chunk(world: &mut impl GenWorld, s: &Settings, chunk_x: i32, chunk_z: i32) {
    for ore in ORES {
        let mut rng = Prng::for_feature(chunk_x, chunk_z, ore);
        if let Some(origin) = feature_origin(&mut rng, ore, chunk_x, chunk_z) {
            place_feature(world, &mut rng, s, ore, origin);
        }
    }
}

// ------------------------------------------------------------------ the recipes

/// What a berry smelts into: the original gives the first item of `forge:nuggets/<metal>`,
/// and has no recipe when that tag is empty. In the game now only iron, gold and copper
/// have a nugget (vanilla's); the other metals' recipes wait for a mod that adds theirs.
pub fn nugget_result(ore: Ore) -> Option<&'static str> {
    match ore {
        Ore::Iron => Some("minecraft:iron_nugget"),
        Ore::Gold => Some("minecraft:gold_nugget"),
        Ore::Copper => Some("minecraft:copper_nugget"),
        _ => None,
    }
}

}
use engine::{Ore, Prng, Settings, Support};
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::registry::BlockStateId;
use std::{cell::RefCell, rc::Rc};

/// The level's random as the world-bound draws use it.
struct Draws<'a>(&'a mut LegacyRandom);
impl engine::Rng for Draws<'_> {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.0.next_int_bound(bound).unwrap_or(0)
    }
    fn next_boolean(&mut self) -> bool {
        self.0.next_boolean()
    }
}

/// The game's states of the bushes: for each state the ore and the age, and the
/// state of each ore at each age.
struct Ids {
    len: usize,
    /// Per state: 0 when it is not a bush, else `1 + ore index` and the age in the
    /// high bits.
    info: Vec<u8>,
    states: [[Option<BlockStateId>; 4]; 12],
    any: bool,
}
impl Ids {
    fn build(game: &Game<'_>) -> Self {
        let registry = game.pack.assets.registry;
        let len = registry.len();
        let mut info = vec![0u8; len];
        let mut states = [[None; 4]; 12];
        let mut any = false;
        for (i, slot) in info.iter_mut().enumerate() {
            let Some(s) = registry.state(BlockStateId(i as u32)) else {
                continue;
            };
            if !s.name.starts_with("oreberriesreplanted:") {
                continue;
            }
            let Some(ore) = Ore::from_bush(&s.name) else {
                continue;
            };
            let age = s
                .properties
                .get("age")
                .and_then(|a| a.parse::<usize>().ok())
                .filter(|a| *a < 4)
                .unwrap_or(0);
            *slot = (1 + ore.index() as u8) | ((age as u8) << 4);
            states[ore.index()][age] = Some(BlockStateId(i as u32));
            any = true;
        }
        Self { len, info, states, any }
    }
    fn is_bush(&self, state: u32) -> bool {
        self.info.get(state as usize).is_some_and(|i| *i != 0)
    }
    /// The ore and the age of a bush state.
    fn bush(&self, state: BlockStateId) -> Option<(Ore, i32)> {
        let i = *self.info.get(state.0 as usize)?;
        if i == 0 {
            return None;
        }
        Some((engine::ORES[usize::from(i & 15) - 1], i32::from(i >> 4)))
    }
    fn state(&self, ore: Ore, age: i32) -> Option<BlockStateId> {
        self.states[ore.index()].get(usize::try_from(age).ok()?).copied().flatten()
    }
}

struct Runtime {
    ids: Option<Rc<Ids>>,
    rng: Prng,
}
thread_local! {
    static RT: RefCell<Runtime> = RefCell::new(Runtime {
        ids: None,
        rng: Prng(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64),
        ),
    });
}

impl Game<'_> {
    fn oreberries_ids(&self) -> Option<Rc<Ids>> {
        let len = self.pack.assets.registry.len();
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            if rt.ids.as_ref().is_none_or(|i| i.len != len) {
                rt.ids = Some(Rc::new(Ids::build(self)));
            }
            rt.ids.clone()
        })
        .filter(|ids| ids.any)
    }

    /// Once a tick, where the game ticks its blocks (`tick_blocks`): the random ticks of
    /// the bushes, and the bushes' touch.
    pub(super) fn oreberries_tick(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(ids) = self.oreberries_ids() else {
            return;
        };
        self.oreberries_grow(&ids);
        self.oreberries_touch(&ids);
    }

    /// `ServerLevel.tickChunk`'s random ticks, for the bushes only: `randomTickSpeed`
    /// positions in each section that holds one, each a tick of the bush on it
    /// (`OreBerryBushBlock.tick`).
    fn oreberries_grow(&mut self, ids: &Ids) {
        let speed = self.ui.rules.int("random_tick_speed");
        if speed <= 0 {
            return;
        }
        let levels = self.tickets.levels();
        let keys: Vec<(i32, i32)> = self
            .world
            .chunks
            .keys()
            .copied()
            .filter(|&(x, z)| levels.random_ticking(x, z) && super::ticking_chunk(&self.world, x, z))
            .collect();
        let mut ticked: Vec<([i32; 3], BlockStateId)> = Vec::new();
        RT.with(|rt| {
            let mut rt = rt.borrow_mut();
            for key in &keys {
                let Some(chunk) = self.world.chunks.get(key) else {
                    continue;
                };
                for (s, section) in chunk.sections.iter().enumerate() {
                    if !section.blocks.maybe_has(|v| ids.is_bush(v)) {
                        continue;
                    }
                    let y0 = horizon_client::chunk::MIN_Y + s as i32 * 16;
                    for _ in 0..speed {
                        let (dx, dy, dz) = {
                            use engine::Rng;
                            (
                                rt.rng.next_int_bound(16),
                                rt.rng.next_int_bound(16),
                                rt.rng.next_int_bound(16),
                            )
                        };
                        let state = chunk.block(dx, y0 + dy, dz);
                        if ids.is_bush(state.0) {
                            ticked.push(([key.0 * 16 + dx, y0 + dy, key.1 * 16 + dz], state));
                        }
                    }
                }
            }
        });
        let settings = Settings::default();
        for (pos, state) in ticked {
            if self.world.block(pos[0], pos[1], pos[2]) != state {
                continue;
            }
            let Some((ore, age)) = ids.bush(state) else {
                continue;
            };
            let light = self.world.light(pos[0], pos[1], pos[2]);
            let brightness = engine::raw_brightness(light >> 4, light & 15);
            let grown = RT.with(|rt| engine::grow(&settings, ore, age, brightness, &mut rt.borrow_mut().rng));
            if let Some(next) = grown.and_then(|a| ids.state(ore, a)) {
                let _ = self.level_set(pos, next);
            }
        }
    }

    /// `OreBerryBushBlock.entityInside` for the player: 1 point of cactus damage for each
    /// tick spent in the block of a bush (the invulnerability frames make it one
    /// hit every half second). Mobs are not hurt here.
    fn oreberries_touch(&mut self, ids: &Ids) {
        let Some(p) = self.player.as_ref() else {
            return;
        };
        if self.spectator() {
            return;
        }
        let w = f64::from(p.dimensions.width) / 2.0;
        let h = f64::from(p.dimensions.height);
        let min = [p.position.x - w, p.position.y, p.position.z - w];
        let max = [p.position.x + w, p.position.y + h, p.position.z + w];
        let touching = engine::cells_touched(min, max)
            .into_iter()
            .any(|c| ids.is_bush(self.world.block(c[0], c[1], c[2]).0));
        if touching {
            self.hurt_player_by(engine::HURT_SOURCE, engine::HURT_AMOUNT);
        }
    }

    /// `harvestBerry` on the bush at `pos`: a grown one goes back one age and gives
    /// 1 to 3 berries. Whether it did.
    fn oreberries_harvest(&mut self, ids: &Ids, pos: [i32; 3]) -> bool {
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let Some((ore, age)) = ids.bush(state) else {
            return false;
        };
        let harvested = {
            let mut draws = Draws(&mut self.world.random);
            engine::harvest(age, &mut draws)
        };
        let Some(h) = harvested else {
            return false;
        };
        let Some(next) = ids.state(ore, h.new_age) else {
            return false;
        };
        let _ = self.level_set(pos, next);
        self.pop_resource(pos, ItemStack::new(&ore.berry(), h.berries as u32));
        true
    }

    /// `OreBerryBushBlock.attack`: the start of a left click on a grown bush.
    pub(super) fn oreberries_attack(&mut self, pos: [i32; 3]) {
        if self.remote.is_some() {
            return;
        }
        let Some(ids) = self.oreberries_ids() else {
            return;
        };
        self.oreberries_harvest(&ids, pos);
    }

    /// Whether the bush item may be put where `BlockItem.place` would put it
    /// (`OreBerryBushBlock.canSurvive`, asked when the block is placed).
    fn oreberries_may_place(&mut self, ids: &Ids, ore: Ore, hit: &super::Hit, item: &str) -> bool {
        let face = match hit.face {
            [0, 1, 0] => Direction::Up,
            [0, -1, 0] => Direction::Down,
            [0, 0, -1] => Direction::North,
            [0, 0, 1] => Direction::South,
            [-1, 0, 0] => Direction::West,
            _ => Direction::East,
        };
        let ctx = horizon_world::level::PlaceContext {
            clicked_pos: horizon_core::position::BlockPos::new(hit.pos[0], hit.pos[1], hit.pos[2]),
            face,
            hit: horizon_core::geometry::Vec3::new(hit.point[0], hit.point[1], hit.point[2]),
            inside: false,
            yaw: self.yaw,
            pitch: self.pitch,
            sneaking: self.keys.contains(&super::KeyCode::ShiftLeft),
            item: item.to_owned(),
            can_use_game_master_blocks: self.can_use_game_master_blocks(),
        };
        let Some((pos, _)) = self.with_level(|b, level, ticks| b.placement_state(level, ticks, &ctx)) else {
            // The game refuses by itself.
            return true;
        };
        let light = self.world.light(pos.x, pos.y, pos.z);
        let brightness = engine::raw_brightness(light >> 4, light & 15);
        let below = self.world.block(pos.x, pos.y - 1, pos.z);
        let support = match ids.bush(below) {
            Some((_, age)) => Support::Bush { age },
            None => Support::Block {
                sturdy_top: self.with_level(|b, _, _| b.is_face_sturdy(below, Direction::Up)),
            },
        };
        engine::can_place(&Settings::default(), ore, brightness, support)
    }

    /// `EssenceBerryItem.use`: an experience orb of 6 to 19 points, of the whole stack's
    /// when sneaking.
    fn oreberries_essence(&mut self) {
        let sneaking = self.keys.contains(&super::KeyCode::ShiftLeft);
        let count = self.held().map_or(1, |s| s.count);
        let xp = {
            use engine::Rng;
            let mut draws = Draws(&mut self.world.random);
            let mut xp = draws.next_int_bound(14) + 6;
            if sneaking {
                xp = 0;
                for _ in 0..count {
                    xp += draws.next_int_bound(14) + 6;
                }
            }
            xp
        };
        if let Some(p) = self.player.as_ref() {
            let at = [p.position.x, p.position.y, p.position.z];
            self.spawn_orbs(at, xp as u32);
        }
        if !self.creative {
            if sneaking {
                *self.held_mut() = None;
            } else {
                self.consume_held();
            }
        }
        self.swing_arm();
    }

    /// `OreBerryBushBlock.use` on the main hand's turn, called where the game tries the
    /// uses of the blocks it knows (`use_composter`, with the clicked block's position): a
    /// grown bush gives its berries. The original answers PASS, so the click goes on.
    pub(super) fn oreberries_click(&mut self, pos: [i32; 3]) {
        if self.remote.is_some() || self.hand != super::Hand::Main {
            return;
        }
        let Some(ids) = self.oreberries_ids() else {
            return;
        };
        if self.oreberries_harvest(&ids, pos) {
            self.swing_arm();
        }
    }

    /// The held item, where the game tries the items that act on their own (`throw_item`, after
    /// the clicked block had its say): the essence berry is used, and a bush item is not placed
    /// where `canSurvive` says no (`FAIL`: the click ends).
    pub(super) fn oreberries_item(&mut self, stack: &ItemStack) -> core::ops::ControlFlow<Result<bool>> {
        use core::ops::ControlFlow::{Break, Continue};
        if self.remote.is_some() || !stack.item.starts_with("oreberriesreplanted:") {
            return Continue(());
        }
        let Some(ids) = self.oreberries_ids() else {
            return Continue(());
        };
        if &*stack.item == "oreberriesreplanted:essence_berry" {
            self.oreberries_essence();
            return Break(Ok(true));
        }
        let Some(ore) = Ore::from_bush(&stack.item) else {
            return Continue(());
        };
        let Some(hit) = self.pick() else {
            return Continue(());
        };
        if self.oreberries_may_place(&ids, ore, &hit, &stack.item) {
            Continue(())
        } else {
            Break(Ok(true))
        }
    }
}
