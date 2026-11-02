// YUNG's Bridges for Horizon: the rules of the mod's Java code, without any
// game code. Written from the mod's source (YUNGNICKYOUNG/YUNGs-Bridges, branch
// 1.20, 4.0.3, LGPL-3.0: world/placement/BridgePlacement.java,
// RngInitializerPlacement.java, world/feature/MultipleAttemptSingleRandomFeature,
// BridgeFeature, AbstractTemplateFeature, world/processor/*) and YUNG's API 4.0
// (BlockStateRandomizer). The numbers of each bridge (length, width, water
// span, solid banks, axis, chance) are read from the mod's data files.
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

/// A block state by name and properties, as the processors build them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub name: String,
    pub props: BTreeMap<String, String>,
}
impl State {
    pub fn of(name: &str) -> Self {
        let name = if name.contains(':') {
            name.to_owned()
        } else {
            format!("minecraft:{name}")
        };
        Self {
            name,
            props: BTreeMap::new(),
        }
    }
    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.props.insert(key.to_owned(), value.to_owned());
        self
    }
    fn is(&self, name: &str) -> bool {
        self.name.strip_prefix("minecraft:") == Some(name)
    }
    fn get(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }
}

/// The level as the feature sees it (a WorldGenRegion). `None`: a block the
/// generator cannot read (outside the chunks around the one being decorated).
pub trait World {
    /// Level.getSeaLevel.
    fn sea_level(&self) -> i32;
    /// BlockState.canOcclude of the block at `p`.
    fn can_occlude(&mut self, p: Pos) -> Option<bool>;
    /// (BlockState.isAir, BlockState.liquid: a water or lava block) at `p`.
    fn air_or_liquid(&mut self, p: Pos) -> Option<(bool, bool)>;
    /// getHeight(WORLD_SURFACE, x, z): the first free Y above the column.
    fn surface(&mut self, x: i32, z: i32) -> Option<i32>;
    /// The block at `p` with its properties.
    fn state(&mut self, p: Pos) -> Option<State>;
    /// setBlock(p, state, 2); the level refuses blocks outside its write area.
    fn set(&mut self, p: Pos, state: &State);
}

/// The feature's random (the decoration's WorldgenRandom).
pub trait Random {
    fn next_int(&mut self, bound: i32) -> i32;
    fn next_float(&mut self) -> f32;
    fn next_long(&mut self) -> i64;
    fn set_seed(&mut self, seed: i64);
}

/// `yungsbridges:bridge` placement (BridgePlacement).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BridgePlacement {
    pub length: i32,
    pub width: i32,
    pub min_water_z: i32,
    pub max_water_z: i32,
    pub width_offset: i32,
    pub num_solid_blocks_needed: i32,
    pub is_z_axis: bool,
}

/// One placement modifier of a bridge's placed feature, in the data's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modifier {
    Bridge(BridgePlacement),
    /// `minecraft:rarity_filter`: `nextFloat() < 1 / chance`.
    Rarity(i32),
    /// `yungsbridges:rng_initializer`.
    RngInitializer,
}

/// `yungsbridges:bridge` feature (BridgeFeatureConfig).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bridge {
    /// The template, `yungsbridges:bridge/stone/15_1`.
    pub location: String,
    pub is_z_axis: bool,
}

/// One entry of `yungsbridges:multiple_attempt_single_random`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub bridge: Bridge,
    pub placement: Vec<Modifier>,
}

/// BridgePlacement.getPositions for the decoration origin `origin`: the corner
/// of the first spot (in the mod's scan order) with land on both banks and only
/// water between them at sea level, or none.
pub fn bridge_position<W: World>(world: &mut W, p: &BridgePlacement, origin: Pos) -> Option<Pos> {
    // The mutable is always at sea level, one below the level's.
    let sea = world.sea_level() - 1;
    let [ox, _, oz] = origin;
    let (length, width, off) = (p.length, p.width, p.width_offset);
    let at = |minor: i32, major: i32| -> Pos {
        if p.is_z_axis {
            [ox + minor, sea, oz + major]
        } else {
            [ox + major, sea, oz + minor]
        }
    };
    // (canOcclude, getHeight(WORLD_SURFACE)) of a sea-level block; the height
    // only when it occludes (the mod's && stops there).
    fn read(world: &mut impl World, q: Pos) -> Option<(bool, i32)> {
        let occludes = world.can_occlude(q)?;
        let surface = if occludes {
            world.surface(q[0], q[2])?
        } else {
            i32::MIN
        };
        Some((occludes, surface))
    }
    // A bank block: it occludes and nothing stands on it higher than sea level.
    let bank = |world: &mut W, q: Pos| -> bool {
        read(world, q).is_some_and(|(occludes, surface)| occludes && surface <= sea + 1)
    };
    let mut minor = width / 2 + off + 1;
    while minor < 16 - width / 2 - off {
        for start in 0..16 {
            let s = at(minor, start);
            let e = at(minor, start + length + 1);
            // canOcclude(start) && canOcclude(end) && height(start) <= sea + 1 && height(end) <= sea + 1.
            let both = read(world, s).zip(read(world, e));
            let Some(((so, sh), (eo, eh))) = both else {
                continue;
            };
            if !(so && eo && sh <= sea + 1 && eh <= sea + 1) {
                continue;
            }
            // Each bank needs solid ground across the bridge's width.
            let banks_hold = [s, e].into_iter().all(|bank_pos| {
                let mut solid = 1;
                for direction in [-1, 1] {
                    for d in 1..=width / 2 {
                        let q = if p.is_z_axis {
                            [bank_pos[0] + direction * d, bank_pos[1], bank_pos[2]]
                        } else {
                            [bank_pos[0], bank_pos[1], bank_pos[2] + direction * d]
                        };
                        if bank(world, q) {
                            solid += 1;
                        } else {
                            break;
                        }
                    }
                }
                solid >= p.num_solid_blocks_needed
            });
            if !banks_hold {
                continue;
            }
            // Only water (a liquid block) between minWaterZ and maxWaterZ.
            let mut water = true;
            'water: for m in -width / 2..=width / 2 {
                for j in p.min_water_z..=p.max_water_z {
                    let q = if p.is_z_axis {
                        [s[0] + m, sea, s[2] + j]
                    } else {
                        [s[0] + j, sea, s[2] + m]
                    };
                    if !world.air_or_liquid(q).is_some_and(|(_, liquid)| liquid) {
                        water = false;
                        break 'water;
                    }
                }
            }
            if water {
                return Some(if p.is_z_axis {
                    [s[0] - width / 2 - off, sea, s[2] + 1]
                } else {
                    [s[0] + 1, sea, s[2] + width / 2 + off]
                });
            }
        }
        minor += 1;
    }
    None
}

/// RngInitializerPlacement: two longs from the random, then a seed of the
/// position mixed with them (Java long arithmetic).
pub fn rng_initializer(random: &mut impl Random, pos: Pos) {
    let a = random.next_long() | 1;
    let b = random.next_long() | 1;
    let x = i64::from(pos[0]);
    let z = i64::from(pos[2]);
    let left = x
        .wrapping_mul(a)
        .wrapping_mul(951_873_395_712)
        .wrapping_add(12_132_586);
    let right = z
        .wrapping_mul(b)
        .wrapping_mul(132_899_567_841)
        .wrapping_add(9_789_717);
    random.set_seed(left.wrapping_mul(right) ^ 313_281_234);
}

/// PlacedFeature.place of one entry: its modifiers in order on the origin,
/// then the feature at the position they leave, if any.
pub fn place_entry<W: World, R: Random>(
    world: &mut W,
    random: &mut R,
    entry: &Entry,
    origin: Pos,
    feature: &mut impl FnMut(&mut W, &mut R, &Bridge, Pos) -> bool,
) -> bool {
    let mut pos = Some(origin);
    for m in &entry.placement {
        let Some(p) = pos else { break };
        pos = match m {
            Modifier::Bridge(b) => bridge_position(world, b, p),
            Modifier::Rarity(chance) => (random.next_float() < 1.0 / *chance as f32).then_some(p),
            Modifier::RngInitializer => {
                rng_initializer(random, p);
                Some(p)
            }
        };
    }
    pos.is_some_and(|p| feature(world, random, &entry.bridge, p))
}

/// MultipleAttemptSingleRandomFeature: entries drawn at random, without
/// putting back, until one places.
pub fn place_list<W: World, R: Random>(
    world: &mut W,
    random: &mut R,
    entries: &[Entry],
    origin: Pos,
    mut feature: impl FnMut(&mut W, &mut R, &Bridge, Pos) -> bool,
) -> bool {
    let mut left: Vec<&Entry> = entries.iter().collect();
    while !left.is_empty() {
        let i = random.next_int(left.len() as i32) as usize;
        if place_entry(world, random, left[i], origin, &mut feature) {
            return true;
        }
        left.remove(i);
    }
    false
}

/// BridgeFeature.place: the template's corner is the placement's position at
/// the level's sea level; bridges along X are turned counterclockwise.
pub fn bridge_corner(world: &impl World, pos: Pos) -> Pos {
    [pos[0], world.sea_level(), pos[2]]
}

/// The wood of the biome at the template's corner
/// (ITemplateFeatureProcessor.get*BiomeVariant, in the mod's order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wood {
    Oak,
    DarkOak,
    Spruce,
    Jungle,
    Acacia,
}
impl Wood {
    /// `#yungsbridges:collections/badlands`, `/taiga`, `/jungle`, `/savanna`.
    pub fn of_biome(badlands: bool, taiga: bool, jungle: bool, savanna: bool) -> Self {
        if badlands {
            Self::DarkOak
        } else if taiga {
            Self::Spruce
        } else if jungle {
            Self::Jungle
        } else if savanna {
            Self::Acacia
        } else {
            Self::Oak
        }
    }
    fn block(self, kind: &str) -> State {
        let wood = match self {
            Self::Oak => "oak",
            Self::DarkOak => "dark_oak",
            Self::Spruce => "spruce",
            Self::Jungle => "jungle",
            Self::Acacia => "acacia",
        };
        State::of(&format!("{wood}_{kind}"))
    }
}

/// YUNG's API BlockStateRandomizer: the entry whose share of [0, 1) holds one
/// nextFloat, else the default.
struct Randomizer {
    default: &'static str,
    entries: &'static [(&'static str, f32)],
}
impl Randomizer {
    fn get(&self, random: &mut impl Random) -> State {
        let target = random.next_float();
        let mut bottom = 0f32;
        for &(name, p) in self.entries {
            if bottom <= target && target < bottom + p {
                return State::of(name);
            }
            bottom += p;
        }
        State::of(self.default)
    }
}

const STONE_BRICK: Randomizer = Randomizer {
    default: "stone_bricks",
    entries: &[("mossy_stone_bricks", 0.5), ("cracked_stone_bricks", 0.25)],
};
const COBBLESTONE: Randomizer = Randomizer {
    default: "cobblestone",
    entries: &[("mossy_cobblestone", 0.6)],
};
const STONE_BRICK_SLAB: Randomizer = Randomizer {
    default: "stone_brick_slab",
    entries: &[("mossy_stone_brick_slab", 0.5)],
};
const STONE_BRICK_WALL: Randomizer = Randomizer {
    default: "stone_brick_wall",
    entries: &[("mossy_stone_brick_wall", 0.5)],
};
const STONE_BRICK_STAIRS: Randomizer = Randomizer {
    default: "stone_brick_stairs",
    entries: &[("mossy_stone_brick_stairs", 0.5)],
};
const LANTERN: Randomizer = Randomizer {
    default: "air",
    entries: &[("lantern", 0.5)],
};
const TORCH: Randomizer = Randomizer {
    default: "air",
    entries: &[("torch", 0.5)],
};
const WALL_TORCH: Randomizer = Randomizer {
    default: "air",
    entries: &[("wall_torch", 0.5)],
};
const END_STONE_BRICK_WALL: Randomizer = Randomizer {
    default: "air",
    entries: &[("stone_brick_wall", 0.5)],
};
const BROWN_GLASS: Randomizer = Randomizer {
    default: "air",
    entries: &[
        ("stone_bricks", 0.1875),
        ("mossy_stone_bricks", 0.375),
        ("cracked_stone_bricks", 0.1875),
    ],
};
const BLUE_GLASS: Randomizer = Randomizer {
    default: "air",
    entries: &[("chiseled_stone_bricks", 0.75)],
};
const RED_GLASS: Randomizer = Randomizer {
    default: "air",
    entries: &[("cobblestone", 0.75)],
};
const END_STONE_BRICKS: Randomizer = Randomizer {
    default: "air",
    entries: &[("polished_andesite", 0.75)],
};
const DARK_PRISMARINE: Randomizer = Randomizer {
    default: "air",
    entries: &[("cobblestone", 0.375), ("andesite", 0.375)],
};
const PURPUR_SLAB: Randomizer = Randomizer {
    default: "air",
    entries: &[("stone_brick_slab", 0.75)],
};
const END_STONE_BRICK_SLAB: Randomizer = Randomizer {
    default: "air",
    entries: &[("polished_andesite_slab", 0.75)],
};
const DARK_PRISMARINE_SLAB: Randomizer = Randomizer {
    default: "air",
    entries: &[("cobblestone_slab", 0.45), ("andesite_slab", 0.45)],
};
const PRISMARINE_BRICK_SLAB: Randomizer = Randomizer {
    default: "air",
    entries: &[("smooth_stone_slab", 0.9)],
};
const WARPED_SLAB: Randomizer = Randomizer {
    default: "air",
    entries: &[("cobblestone_slab", 0.9)],
};
const PURPUR_STAIRS: Randomizer = Randomizer {
    default: "air",
    entries: &[("stone_brick_stairs", 0.75)],
};
const DARK_PRISMARINE_STAIRS: Randomizer = Randomizer {
    default: "air",
    entries: &[("cobblestone_stairs", 0.375), ("andesite_stairs", 0.375)],
};
const WARPED_STAIRS: Randomizer = Randomizer {
    default: "air",
    entries: &[("cobblestone_stairs", 0.75)],
};

/// The block families the processors tell apart by class (`instanceof`).
fn family(name: &str) -> Option<&'static str> {
    let path = name.strip_prefix("minecraft:").unwrap_or(name);
    if path.ends_with("_stairs") {
        Some("stairs")
    } else if path.ends_with("_slab") {
        Some("slab")
    } else if path.ends_with("_wall") {
        Some("wall")
    } else if path.ends_with("_fence") {
        Some("fence")
    } else if path.ends_with("_log") {
        Some("log")
    } else if path == "lantern" || path == "soul_lantern" {
        Some("lantern")
    } else {
        None
    }
}

/// get*BlockWithState: `input` with the listed properties of `source` (each
/// with the mod's default when the source has none), when `input` is of the
/// family; any other block as it is.
fn with_state(input: State, source: &State, kind: &str) -> State {
    let keys: &[(&str, &str)] = match kind {
        "stairs" => &[
            ("facing", "north"),
            ("half", "bottom"),
            ("shape", "straight"),
            ("waterlogged", "false"),
        ],
        "slab" => &[("type", "bottom"), ("waterlogged", "false")],
        "wall" => &[
            ("north", "none"),
            ("east", "none"),
            ("south", "none"),
            ("west", "none"),
            ("up", "true"),
            ("waterlogged", "false"),
        ],
        "fence" => &[
            ("north", "false"),
            ("east", "false"),
            ("south", "false"),
            ("west", "false"),
            ("waterlogged", "false"),
        ],
        "log" => &[("axis", "y")],
        "lantern" => &[("hanging", "true"), ("waterlogged", "true")],
        _ => &[],
    };
    if family(&input.name) != Some(kind) {
        return input;
    }
    // getWallBlockWithState also wants the source to be a wall.
    if kind == "wall" && family(&source.name) != Some("wall") {
        return input;
    }
    let mut out = input;
    for &(key, default) in keys {
        // The source's value only when it is the same property (a wall's
        // "north" is not a fence's).
        let value = source
            .get(key)
            .filter(|v| match (kind, key) {
                ("wall", "north" | "east" | "south" | "west") => {
                    matches!(*v, "none" | "low" | "tall")
                }
                ("fence", _) | (_, "waterlogged" | "up" | "hanging") => {
                    matches!(*v, "true" | "false")
                }
                _ => true,
            })
            .unwrap_or(default);
        out.props.insert(key.to_owned(), value.to_owned());
    }
    out
}

/// ITemplateFeatureProcessor.generatePillarDown: the starter block becomes
/// `replacement`, then a column of `leg` down through air and liquids while
/// above y = 0. A block the level cannot read ends the column.
fn pillar_down<W: World, R: Random>(
    world: &mut W,
    random: &mut R,
    pos: Pos,
    replacement: &mut impl FnMut(&mut R) -> State,
    leg: &mut impl FnMut(&mut R) -> State,
) {
    let first = replacement(random);
    world.set(pos, &first);
    let mut at = [pos[0], pos[1] - 1, pos[2]];
    while at[1] > 0 && world.air_or_liquid(at).is_some_and(|(air, liquid)| air || liquid) {
        let block = leg(random);
        world.set(at, &block);
        at[1] -= 1;
    }
}

/// The processors of BridgeFeature, in its order, after the template is
/// placed. `blocks` are the template's blocks as `filterBlocks` lists them:
/// palette order, world positions, states turned with the template.
pub fn process<W: World, R: Random>(world: &mut W, random: &mut R, blocks: &[(Pos, State)], wood: Wood) {
    let of = |name: &'static str| blocks.iter().filter(move |(_, s)| s.is(name));

    // DynamicLegProcessor.
    for (p, s) in of("yellow_stained_glass") {
        let log = || with_state(wood.block("log"), s, "log");
        pillar_down(world, random, *p, &mut |_| log(), &mut |_| log());
    }
    for (p, s) in of("warped_fence") {
        let fence = || with_state(wood.block("fence"), s, "fence");
        pillar_down(world, random, *p, &mut |_| fence(), &mut |_| fence());
    }
    for (p, _) in of("pink_stained_glass") {
        pillar_down(world, random, *p, &mut |r| STONE_BRICK.get(r), &mut |r| STONE_BRICK.get(r));
    }
    for (p, _) in of("light_blue_stained_glass") {
        let andesite = |_: &mut R| State::of("polished_andesite");
        pillar_down(world, random, *p, &mut { andesite }, &mut { andesite });
    }
    for (p, _) in of("magenta_stained_glass") {
        pillar_down(world, random, *p, &mut |r| COBBLESTONE.get(r), &mut |r| COBBLESTONE.get(r));
    }
    for (p, _) in of("gray_stained_glass") {
        pillar_down(
            world,
            random,
            *p,
            &mut |_| State::of("polished_andesite"),
            &mut |r| STONE_BRICK.get(r),
        );
    }
    for (p, s) in of("prismarine_wall") {
        // The legs are getFenceBlockWithState of a wall: a plain cobblestone wall.
        pillar_down(
            world,
            random,
            *p,
            &mut |_| with_state(State::of("cobblestone_wall"), s, "wall"),
            &mut |_| with_state(State::of("cobblestone_wall"), s, "fence"),
        );
    }
    // LogBiomeProcessor, StairBiomeProcessor, PlanksBiomeProcessor, SlabBiomeProcessor.
    for (p, s) in of("oak_log") {
        world.set(*p, &with_state(wood.block("log"), s, "log"));
    }
    for (p, s) in of("oak_stairs") {
        world.set(*p, &with_state(wood.block("stairs"), s, "stairs"));
    }
    for (p, _) in of("oak_planks") {
        world.set(*p, &wood.block("planks"));
    }
    for (p, s) in of("oak_slab") {
        world.set(*p, &with_state(wood.block("slab"), s, "slab"));
    }
    // StoneVariationProcessor.
    for (p, _) in of("stone_bricks") {
        let b = STONE_BRICK.get(random);
        world.set(*p, &b);
    }
    for (p, s) in of("stone_brick_slab") {
        let b = STONE_BRICK_SLAB.get(random);
        world.set(*p, &with_state(b, s, "slab"));
    }
    for (p, s) in of("stone_brick_wall") {
        let b = STONE_BRICK_WALL.get(random);
        world.set(*p, &with_state(b, s, "wall"));
    }
    for (p, s) in of("stone_brick_stairs") {
        let b = STONE_BRICK_STAIRS.get(random);
        world.set(*p, &with_state(b, s, "stairs"));
    }
    for (p, _) in of("cobblestone") {
        let b = COBBLESTONE.get(random);
        world.set(*p, &b);
    }
    // LanternRotProcessor (wall torches keep the default facing, as in the mod).
    for (p, s) in of("lantern") {
        let b = LANTERN.get(random);
        world.set(*p, &with_state(b, s, "lantern"));
    }
    for (p, _) in of("torch") {
        let b = TORCH.get(random);
        world.set(*p, &b);
    }
    for (p, _) in of("wall_torch") {
        let b = WALL_TORCH.get(random);
        world.set(*p, &b);
    }
    // OptionalWallProcessor.
    for (p, s) in of("end_stone_brick_wall") {
        let b = END_STONE_BRICK_WALL.get(random);
        world.set(*p, &with_state(b, s, "wall"));
    }
    // OptionalBlockProcessor.
    for (name, randomizer) in [
        ("brown_stained_glass", &BROWN_GLASS),
        ("blue_stained_glass", &BLUE_GLASS),
        ("red_stained_glass", &RED_GLASS),
        ("end_stone_bricks", &END_STONE_BRICKS),
        ("dark_prismarine", &DARK_PRISMARINE),
    ] {
        for (p, _) in of(name) {
            let b = randomizer.get(random);
            world.set(*p, &b);
        }
    }
    for (p, s) in of("lime_stained_glass") {
        let b = if random.next_float() < 0.5 {
            with_state(wood.block("log"), s, "log")
        } else {
            State::of("air")
        };
        world.set(*p, &b);
    }
    // OptionalSlabProcessor.
    for (name, randomizer) in [
        ("purpur_slab", &PURPUR_SLAB),
        ("end_stone_brick_slab", &END_STONE_BRICK_SLAB),
        ("dark_prismarine_slab", &DARK_PRISMARINE_SLAB),
        ("prismarine_brick_slab", &PRISMARINE_BRICK_SLAB),
    ] {
        for (p, s) in of(name) {
            let b = randomizer.get(random);
            world.set(*p, &with_state(b, s, "slab"));
        }
    }
    for (p, s) in of("crimson_slab") {
        let b = if random.next_float() < 0.5 {
            with_state(wood.block("slab"), s, "slab")
        } else {
            State::of("air")
        };
        world.set(*p, &b);
    }
    for (p, s) in of("warped_slab") {
        let b = WARPED_SLAB.get(random);
        world.set(*p, &with_state(b, s, "slab"));
    }
    // OptionalStairProcessor.
    for (name, randomizer) in [
        ("purpur_stairs", &PURPUR_STAIRS),
        ("dark_prismarine_stairs", &DARK_PRISMARINE_STAIRS),
        ("warped_stairs", &WARPED_STAIRS),
    ] {
        for (p, s) in of(name) {
            let b = randomizer.get(random);
            world.set(*p, &with_state(b, s, "stairs"));
        }
    }
    for (p, s) in of("crimson_stairs") {
        world.set(*p, &with_state(wood.block("stairs"), s, "stairs"));
    }
    // FenceBiomeProcessor.
    for (p, s) in of("oak_fence") {
        let above = [p[0], p[1] + 1, p[2]];
        let north = [p[0], p[1], p[2] - 1];
        let west = [p[0] - 1, p[1], p[2]];
        // The float is drawn first, the block above asked only when it fails.
        let keep = random.next_float() < 0.75 || world.can_occlude(above).unwrap_or(false);
        if keep {
            let mut fence = with_state(wood.block("fence"), s, "fence");
            if !world.can_occlude(north).unwrap_or(false) {
                fence.props.insert("north".into(), "false".into());
            }
            if !world.can_occlude(west).unwrap_or(false) {
                fence.props.insert("west".into(), "false".into());
            }
            world.set(*p, &fence);
        } else {
            world.set(*p, &State::of("air"));
            for (neighbour, side) in [(north, "south"), (west, "east")] {
                if let Some(mut n) = world.state(neighbour)
                    && n.get(side) == Some("true")
                {
                    n.props.insert(side.into(), "false".into());
                    world.set(neighbour, &n);
                }
            }
        }
    }
}
