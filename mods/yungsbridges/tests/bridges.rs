use horizon_yungsbridges::*;
use std::collections::{BTreeMap, VecDeque};

/// A level of named blocks: stone up to `ground` (y 62, sea level - 1) except
/// where water is set, air above; `readable` bounds what the "region" shows.
struct Level {
    blocks: BTreeMap<Pos, State>,
    ground: i32,
    readable: Option<([i32; 2], [i32; 2])>,
    writes: Vec<(Pos, State)>,
}
impl Level {
    fn new() -> Self {
        Self {
            blocks: BTreeMap::new(),
            ground: 62,
            readable: None,
            writes: Vec::new(),
        }
    }
    fn put(&mut self, p: Pos, name: &str) {
        self.blocks.insert(p, State::of(name));
    }
    fn reads(&self, p: Pos) -> bool {
        self.readable
            .is_none_or(|(lo, hi)| p[0] >= lo[0] && p[0] <= hi[0] && p[2] >= lo[1] && p[2] <= hi[1])
    }
    fn get(&self, p: Pos) -> State {
        self.blocks.get(&p).cloned().unwrap_or_else(|| {
            State::of(if p[1] <= self.ground { "stone" } else { "air" })
        })
    }
    /// A river along X: water at y 62 (and two below) for z in `z0..=z1`.
    fn river_x(&mut self, z0: i32, z1: i32, xs: std::ops::RangeInclusive<i32>) {
        for x in xs {
            for z in z0..=z1 {
                for y in 58..=62 {
                    self.put([x, y, z], "water");
                }
            }
        }
    }
}
fn occludes(s: &State) -> bool {
    !matches!(
        s.name.as_str(),
        "minecraft:air" | "minecraft:water" | "minecraft:lava" | "minecraft:oak_leaves"
    ) && !s.name.ends_with("_glass")
}
impl World for Level {
    fn sea_level(&self) -> i32 {
        63
    }
    fn can_occlude(&mut self, p: Pos) -> Option<bool> {
        self.reads(p).then(|| occludes(&self.get(p)))
    }
    fn air_or_liquid(&mut self, p: Pos) -> Option<(bool, bool)> {
        self.reads(p).then(|| {
            let s = self.get(p);
            (
                s.name == "minecraft:air",
                matches!(s.name.as_str(), "minecraft:water" | "minecraft:lava"),
            )
        })
    }
    fn surface(&mut self, x: i32, z: i32) -> Option<i32> {
        if !self.reads([x, 0, z]) {
            return None;
        }
        (-64..320)
            .rev()
            .find(|&y| self.get([x, y, z]).name != "minecraft:air")
            .map(|y| y + 1)
    }
    fn state(&mut self, p: Pos) -> Option<State> {
        self.reads(p).then(|| self.get(p))
    }
    fn set(&mut self, p: Pos, s: &State) {
        self.writes.push((p, s.clone()));
        self.blocks.insert(p, s.clone());
    }
}

/// A scripted random: floats, ints and longs in order, with a log of calls.
#[derive(Default)]
struct Script {
    floats: VecDeque<f32>,
    ints: VecDeque<i32>,
    longs: VecDeque<i64>,
    log: Vec<String>,
}
impl Random for Script {
    fn next_int(&mut self, bound: i32) -> i32 {
        self.log.push(format!("int {bound}"));
        self.ints.pop_front().unwrap_or(0)
    }
    fn next_float(&mut self) -> f32 {
        self.log.push("float".into());
        self.floats.pop_front().unwrap_or(0.0)
    }
    fn next_long(&mut self) -> i64 {
        self.log.push("long".into());
        self.longs.pop_front().unwrap_or(0)
    }
    fn set_seed(&mut self, seed: i64) {
        self.log.push(format!("seed {seed}"));
    }
}

fn stone_15_1(is_z_axis: bool) -> BridgePlacement {
    // data/yungsbridges/worldgen/configured_feature/bridge_list.json, stone/15_1.
    BridgePlacement {
        length: 15,
        width: 5,
        min_water_z: 2,
        max_water_z: 14,
        width_offset: 1,
        num_solid_blocks_needed: 2,
        is_z_axis,
    }
}

#[test]
fn finds_the_first_spot_in_scan_order() {
    // Chunk (0, 0); a river along X covering z 3..=16.
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    let p = stone_15_1(true);
    // Major offset 0 ends in the river (z 16); offset 1 has land at 1 and 17
    // and water 3..=15; the first minor offset is width / 2 + offset + 1 = 4.
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([1, 62, 2]));
    // The same river reached from a chunk further along X.
    assert_eq!(
        bridge_position(&mut level, &p, [16, -64, 0]),
        Some([17, 62, 2])
    );
}

#[test]
fn rotated_bridges_cross_rivers_along_z() {
    let mut level = Level::new();
    // A river along Z covering x 3..=16.
    for z in -16..=31 {
        for x in 3..=16 {
            for y in 58..=62 {
                level.put([x, y, z], "water");
            }
        }
    }
    let p = stone_15_1(false);
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([2, 62, 7]));
    // A Z-axis bridge finds nothing across it.
    assert_eq!(bridge_position(&mut level, &stone_15_1(true), [0, -64, 0]), None);
}

#[test]
fn banks_must_be_low_solid_and_wide() {
    let p = stone_15_1(true);
    // A tree on the near bank at the first spot: that spot is refused, the
    // next major offset (z 2) is taken.
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    level.put([4, 63, 1], "oak_log");
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([1, 62, 3]));
    // Leaves stand on the bank too (WORLD_SURFACE counts them).
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    level.put([4, 63, 17], "oak_leaves");
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([1, 62, 3]));
    // Two solid blocks are needed on each bank: with the bank's sides at x 3
    // and 5 dug out only the centre counts.
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    level.put([3, 62, 1], "water");
    level.put([5, 62, 1], "water");
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([1, 62, 3]));
}

#[test]
fn every_block_between_must_be_liquid() {
    let p = stone_15_1(true);
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    // An island in the river under every minor offset's span.
    for x in 0..16 {
        level.put([x, 62, 10], "sand");
    }
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), None);
    // Lava counts as liquid; a waterlogged block (seagrass) does not.
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    level.put([4, 62, 8], "lava");
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([1, 62, 2]));
    // Seagrass at x 4: every span over x 4 (minor 4 to 6) fails, minor 7 holds.
    level.put([4, 62, 9], "seagrass");
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([4, 62, 2]));
}

#[test]
fn an_unreadable_far_bank_is_no_bank() {
    // The decoration sees chunks -1..=1 only (x, z in -16..=31). A bridge of
    // 16 starting at z 15 would end its check at z 32.
    let p = BridgePlacement {
        length: 16,
        width: 5,
        min_water_z: 2,
        max_water_z: 15,
        width_offset: 0,
        num_solid_blocks_needed: 2,
        is_z_axis: true,
    };
    let mut level = Level::new();
    level.river_x(17, 31, -16..=47);
    level.readable = Some(([-16, -16], [31, 31]));
    // z 15 is the only start with water 17..=30 and land at 15 and 32.
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), None);
    level.readable = None;
    assert_eq!(bridge_position(&mut level, &p, [0, -64, 0]), Some([1, 62, 16]));
}

#[test]
fn rng_initializer_mixes_like_java_longs() {
    let mut r = Script::default();
    r.longs = VecDeque::from([6, -8]);
    rng_initializer(&mut r, [100, 63, -37]);
    // a = 6|1 = 7, b = -8|1 = -7, computed with Java's wrapping long arithmetic:
    // ((100*7*951873395712 + 12132586) * (-37*-7*132899567841 + 9789717)) ^ 313281234
    let left = 100i64 * 7 * 951_873_395_712 + 12_132_586;
    let right = (-37i64) * (-7) * 132_899_567_841 + 9_789_717;
    let seed = left.wrapping_mul(right) ^ 313_281_234;
    assert_eq!(r.log, vec!["long", "long", &format!("seed {seed}")]);
    // Large coordinates overflow as Java does.
    let mut r = Script::default();
    r.longs = VecDeque::from([i64::MAX, i64::MIN]);
    rng_initializer(&mut r, [29_999_984, 63, -29_999_984]);
    let left = 29_999_984i64
        .wrapping_mul(i64::MAX)
        .wrapping_mul(951_873_395_712)
        .wrapping_add(12_132_586);
    let right = (-29_999_984i64)
        .wrapping_mul(i64::MIN | 1)
        .wrapping_mul(132_899_567_841)
        .wrapping_add(9_789_717);
    assert_eq!(r.log[2], format!("seed {}", left.wrapping_mul(right) ^ 313_281_234));
}

fn entry(location: &str, p: BridgePlacement) -> Entry {
    Entry {
        bridge: Bridge {
            location: location.into(),
            is_z_axis: p.is_z_axis,
        },
        placement: vec![
            Modifier::Bridge(p),
            Modifier::Rarity(3),
            Modifier::RngInitializer,
        ],
    }
}

#[test]
fn attempts_draw_without_putting_back_until_one_places() {
    let mut level = Level::new();
    level.river_x(3, 16, -16..=31);
    let along_z = entry("yungsbridges:bridge/stone/15_1", stone_15_1(true));
    let along_x = entry("yungsbridges:bridge/stone/15_1_rotated", stone_15_1(false));
    let entries = vec![along_x.clone(), along_z.clone()];
    // First draw: index 0 (along X: no spot, no float); then index 0 of the
    // one left (along Z): a spot, the rarity float 0.2 < 1/3 passes, the
    // initializer's two longs and seed, then the feature.
    let mut r = Script::default();
    r.ints = VecDeque::from([0, 0]);
    r.floats = VecDeque::from([0.2]);
    let mut placed = Vec::new();
    let ok = place_list(&mut level, &mut r, &entries, [0, -64, 0], |_, _, b, p| {
        placed.push((b.location.clone(), p));
        true
    });
    assert!(ok);
    assert_eq!(placed, vec![("yungsbridges:bridge/stone/15_1".into(), [1, 62, 2])]);
    assert_eq!(&r.log[..4], &["int 2", "int 1", "float", "long"]);
    assert!(r.log[5].starts_with("seed "));
    assert_eq!(r.log.len(), 6);

    // The rarity fails (0.5): that entry goes, nothing is left to draw.
    let mut r = Script::default();
    r.ints = VecDeque::from([1, 0]);
    r.floats = VecDeque::from([0.5]);
    let ok = place_list(&mut level, &mut r, &entries, [0, -64, 0], |_, _, _, _| true);
    assert!(!ok);
    assert_eq!(r.log, vec!["int 2", "float", "int 1"]);

    // A missing template (the feature fails) also moves on to the next entry.
    let mut r = Script::default();
    r.ints = VecDeque::from([1, 0]);
    r.floats = VecDeque::from([0.1]);
    let ok = place_list(&mut level, &mut r, &entries, [0, -64, 0], |_, _, _, _| false);
    assert!(!ok);
    assert_eq!(r.log[..2], ["int 2", "float"]);
    assert_eq!(r.log.last().unwrap(), "int 1");
}

#[test]
fn corner_is_at_sea_level() {
    let level = Level::new();
    assert_eq!(bridge_corner(&level, [5, 62, 9]), [5, 63, 9]);
}

fn names(level: &Level, p: Pos) -> String {
    let s = level.get(p);
    let props: Vec<String> = s.props.iter().map(|(k, v)| format!("{k}={v}")).collect();
    if props.is_empty() {
        s.name.clone()
    } else {
        format!("{}[{}]", s.name, props.join(","))
    }
}

#[test]
fn legs_run_down_through_water_and_air() {
    let mut level = Level::new();
    level.river_x(0, 3, 0..=3);
    // Yellow glass: a log leg of the biome's wood with the glass's axis (y).
    let blocks = vec![
        ([1, 63, 1], State::of("yellow_stained_glass")),
        ([2, 63, 1], State::of("light_blue_stained_glass")),
    ];
    let mut r = Script::default();
    process(&mut level, &mut r, &blocks, Wood::Spruce);
    assert_eq!(names(&level, [1, 63, 1]), "minecraft:spruce_log[axis=y]");
    // Air at 63 is the starter itself; the water 58..=62 becomes logs, the stone at 57 stops it.
    for y in 58..=62 {
        assert_eq!(names(&level, [1, y, 1]), "minecraft:spruce_log[axis=y]");
    }
    assert_eq!(names(&level, [1, 57, 1]), "minecraft:stone");
    assert_eq!(names(&level, [2, 60, 1]), "minecraft:polished_andesite");
    assert!(r.log.is_empty());
}

#[test]
fn stone_legs_draw_one_float_per_block() {
    let mut level = Level::new();
    level.river_x(0, 0, 0..=0);
    // 5 water blocks under the starter: 1 + 5 draws for a pink glass leg.
    let blocks = vec![([0, 63, 0], State::of("pink_stained_glass"))];
    let mut r = Script::default();
    r.floats = VecDeque::from([0.1, 0.6, 0.8, 0.49, 0.75, 0.74]);
    process(&mut level, &mut r, &blocks, Wood::Oak);
    assert_eq!(r.log.len(), 6);
    // mossy below 0.5, cracked in [0.5, 0.75), plain from 0.75.
    assert_eq!(names(&level, [0, 63, 0]), "minecraft:mossy_stone_bricks");
    assert_eq!(names(&level, [0, 62, 0]), "minecraft:cracked_stone_bricks");
    assert_eq!(names(&level, [0, 61, 0]), "minecraft:stone_bricks");
    assert_eq!(names(&level, [0, 60, 0]), "minecraft:mossy_stone_bricks");
    assert_eq!(names(&level, [0, 59, 0]), "minecraft:stone_bricks");
    assert_eq!(names(&level, [0, 58, 0]), "minecraft:cracked_stone_bricks");
}

#[test]
fn prismarine_wall_legs_are_plain_cobblestone_walls() {
    let mut level = Level::new();
    level.river_x(0, 0, 0..=0);
    let wall = State::of("prismarine_wall")
        .with("north", "low")
        .with("east", "none")
        .with("south", "tall")
        .with("west", "none")
        .with("up", "true")
        .with("waterlogged", "false");
    process(&mut level, &mut Script::default(), &[([0, 63, 0], wall)], Wood::Oak);
    assert_eq!(
        names(&level, [0, 63, 0]),
        "minecraft:cobblestone_wall[east=none,north=low,south=tall,up=true,waterlogged=false,west=none]"
    );
    // getFenceBlockWithState of a wall leaves the wall's defaults.
    assert_eq!(names(&level, [0, 62, 0]), "minecraft:cobblestone_wall");
}

#[test]
fn wood_follows_the_biome_and_keeps_the_shape() {
    assert_eq!(Wood::of_biome(true, true, true, true), Wood::DarkOak);
    assert_eq!(Wood::of_biome(false, true, true, true), Wood::Spruce);
    assert_eq!(Wood::of_biome(false, false, true, true), Wood::Jungle);
    assert_eq!(Wood::of_biome(false, false, false, true), Wood::Acacia);
    assert_eq!(Wood::of_biome(false, false, false, false), Wood::Oak);
    let mut level = Level::new();
    let stairs = State::of("oak_stairs")
        .with("facing", "west")
        .with("half", "top")
        .with("shape", "outer_left")
        .with("waterlogged", "true");
    let blocks = vec![
        ([0, 64, 0], State::of("oak_log").with("axis", "x")),
        ([1, 64, 0], stairs),
        ([2, 64, 0], State::of("oak_planks")),
        ([3, 64, 0], State::of("oak_slab").with("type", "top").with("waterlogged", "false")),
    ];
    process(&mut level, &mut Script::default(), &blocks, Wood::Acacia);
    assert_eq!(names(&level, [0, 64, 0]), "minecraft:acacia_log[axis=x]");
    assert_eq!(
        names(&level, [1, 64, 0]),
        "minecraft:acacia_stairs[facing=west,half=top,shape=outer_left,waterlogged=true]"
    );
    assert_eq!(names(&level, [2, 64, 0]), "minecraft:acacia_planks");
    assert_eq!(
        names(&level, [3, 64, 0]),
        "minecraft:acacia_slab[type=top,waterlogged=false]"
    );
}

#[test]
fn processors_run_in_the_mods_order() {
    // One block of each random processor; the floats are taken in the order
    // of BridgeFeature's processor list and of each processor's blocks.
    let mut level = Level::new();
    let at = |i: i32| [i, 70, 0];
    let blocks = vec![
        (at(0), State::of("oak_fence")),
        (at(1), State::of("cobblestone")),
        (at(2), State::of("lantern").with("hanging", "true").with("waterlogged", "false")),
        (at(3), State::of("stone_bricks")),
        (at(4), State::of("lime_stained_glass")),
        (at(5), State::of("crimson_slab").with("type", "bottom")),
        (at(6), State::of("brown_stained_glass")),
        (at(7), State::of("end_stone_brick_wall")),
        (at(8), State::of("torch")),
        (at(9), State::of("purpur_stairs").with("facing", "east")),
    ];
    let mut r = Script::default();
    // stone bricks .3 (mossy), cobblestone .7 (plain), lantern .2 (lantern),
    // torch .9 (air), end stone wall .1 (stone brick wall), brown glass .5
    // (mossy: .1875 <= .5 < .5625), lime .6 (air), crimson slab .4 (oak slab),
    // purpur stairs .8 (air), fence .9 (no fence, nothing above).
    r.floats = VecDeque::from([0.3, 0.7, 0.2, 0.9, 0.1, 0.5, 0.6, 0.4, 0.8, 0.9]);
    process(&mut level, &mut r, &blocks, Wood::Oak);
    assert_eq!(r.log.len(), 10);
    assert_eq!(names(&level, at(3)), "minecraft:mossy_stone_bricks");
    assert_eq!(names(&level, at(1)), "minecraft:cobblestone");
    assert_eq!(
        names(&level, at(2)),
        "minecraft:lantern[hanging=true,waterlogged=false]"
    );
    assert_eq!(names(&level, at(8)), "minecraft:air");
    assert_eq!(names(&level, at(7)).split('[').next().unwrap(), "minecraft:stone_brick_wall");
    assert_eq!(names(&level, at(6)), "minecraft:mossy_stone_bricks");
    assert_eq!(names(&level, at(4)), "minecraft:air");
    assert_eq!(
        names(&level, at(5)),
        "minecraft:oak_slab[type=bottom,waterlogged=false]"
    );
    assert_eq!(names(&level, at(9)), "minecraft:air");
    assert_eq!(names(&level, at(0)), "minecraft:air");
}

#[test]
fn fences_connect_only_to_what_occludes() {
    // Kept (float .1): north and west turned off where nothing occludes.
    let mut level = Level::new();
    level.put([0, 70, -1], "air");
    level.put([-1, 70, 0], "stone");
    let fence = State::of("oak_fence")
        .with("north", "true")
        .with("east", "true")
        .with("south", "false")
        .with("west", "true")
        .with("waterlogged", "false");
    let mut r = Script::default();
    r.floats = VecDeque::from([0.1]);
    process(&mut level, &mut r, &[([0, 70, 0], fence.clone())], Wood::Jungle);
    assert_eq!(
        names(&level, [0, 70, 0]),
        "minecraft:jungle_fence[east=true,north=false,south=false,waterlogged=false,west=true]"
    );
    // Taken out (float .8, nothing above): the neighbours stop pointing at it.
    let mut level = Level::new();
    level.blocks.insert([0, 70, -1], State::of("oak_fence").with("south", "true"));
    level.blocks.insert([-1, 70, 0], State::of("oak_fence").with("east", "true"));
    let mut r = Script::default();
    r.floats = VecDeque::from([0.8]);
    process(&mut level, &mut r, &[([0, 70, 0], fence.clone())], Wood::Oak);
    assert_eq!(names(&level, [0, 70, 0]), "minecraft:air");
    assert_eq!(names(&level, [0, 70, -1]), "minecraft:oak_fence[south=false]");
    assert_eq!(names(&level, [-1, 70, 0]), "minecraft:oak_fence[east=false]");
    // Float .8 but a block above that occludes: the fence stays.
    let mut level = Level::new();
    level.put([0, 71, 0], "oak_planks");
    let mut r = Script::default();
    r.floats = VecDeque::from([0.8]);
    process(&mut level, &mut r, &[([0, 70, 0], fence)], Wood::Oak);
    assert!(names(&level, [0, 70, 0]).starts_with("minecraft:oak_fence["));
}
