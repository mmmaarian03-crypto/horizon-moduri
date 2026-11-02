// Tests of the Oreberries Replanted rules. The numbers are the ones of the 1.20.1 source
// (OreBerryBushBlock, OreBerryBushFeature, OreBerryFeatures, OreBerryPlacements) and of the
// pack's config.
use horizon_oreberriesreplanted::*;
use std::collections::HashMap;

/// Scripted draws: bounded ints and booleans in the order they are asked.
struct Script {
    ints: Vec<i32>,
    bools: Vec<bool>,
    asked: Vec<i32>,
    asked_bools: usize,
}
impl Script {
    fn new(ints: &[i32], bools: &[bool]) -> Self {
        Self {
            ints: ints.to_vec(),
            bools: bools.to_vec(),
            asked: Vec::new(),
            asked_bools: 0,
        }
    }
}
impl Rng for Script {
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.asked.push(bound);
        self.ints.remove(0)
    }
    fn next_boolean(&mut self) -> bool {
        self.asked_bools += 1;
        self.bools.remove(0)
    }
}

// ------------------------------------------------------------------ the bush

#[test]
fn the_pack_has_the_default_config() {
    let s = Settings::default();
    assert_eq!(s.growth_chance, 20);
    assert!(ORES.iter().all(|o| s.dark_only(*o)));
    let density: Vec<i32> = ORES.iter().map(|o| s.density(*o)).collect();
    // oreberriesreplanted-common.toml: iron 1, gold 1, copper 2, tin 2, aluminum 2, lead 1,
    // nickel 1, uranium 1, osmium 1, zinc 2, silver 1, essence 2.
    assert_eq!(density, vec![1, 1, 2, 2, 2, 1, 1, 1, 1, 2, 1, 2]);
}

#[test]
fn names_follow_the_registry() {
    assert_eq!(Ore::Iron.bush(), "oreberriesreplanted:iron_oreberry_bush");
    assert_eq!(Ore::Iron.berry(), "oreberriesreplanted:iron_oreberry");
    assert_eq!(Ore::Essence.bush(), "oreberriesreplanted:essence_berry_bush");
    assert_eq!(Ore::Essence.berry(), "oreberriesreplanted:essence_berry");
    assert_eq!(Ore::from_bush("oreberriesreplanted:zinc_oreberry_bush"), Some(Ore::Zinc));
    assert_eq!(Ore::from_bush("oreberriesreplanted:oak_vat"), None);
    assert_eq!(ORES.len(), 12);
}

#[test]
fn a_bush_grows_one_time_in_twenty_and_only_in_the_dark() {
    let s = Settings::default();
    // The draw is 0 (one in 20) and the light is 9: it grows.
    let mut r = Script::new(&[0], &[]);
    assert_eq!(grow(&s, Ore::Iron, 0, 9, &mut r), Some(1));
    assert_eq!(r.asked, vec![20]);
    // Brightness 10 is too bright, but the draw was made all the same.
    let mut r = Script::new(&[0], &[]);
    assert_eq!(grow(&s, Ore::Iron, 1, 10, &mut r), None);
    assert_eq!(r.asked, vec![20]);
    // Any other draw: no growth, light does not matter.
    let mut r = Script::new(&[7], &[]);
    assert_eq!(grow(&s, Ore::Gold, 2, 0, &mut r), None);
    // The last age does not draw.
    let mut r = Script::new(&[], &[]);
    assert_eq!(grow(&s, Ore::Iron, 3, 0, &mut r), None);
    assert!(r.asked.is_empty());
}

#[test]
fn growth_follows_the_options() {
    let mut s = Settings::default();
    s.growth_chance = 1;
    let mut r = Script::new(&[0], &[]);
    assert_eq!(grow(&s, Ore::Tin, 0, 5, &mut r), Some(1));
    assert_eq!(r.asked, vec![1]);
    // darknessOnlyTinBush = false: the light is not looked at.
    s.darkness_only[Ore::Tin.index()] = false;
    let mut r = Script::new(&[0], &[]);
    assert_eq!(grow(&s, Ore::Tin, 0, 15, &mut r), Some(1));
    // The others still need the dark.
    let mut r = Script::new(&[0], &[]);
    assert_eq!(grow(&s, Ore::Iron, 0, 15, &mut r), None);
}

#[test]
fn only_a_grown_bush_is_harvested_and_it_goes_back_one_age() {
    let mut r = Script::new(&[2], &[]);
    let h = harvest(3, &mut r).unwrap();
    assert_eq!(h, Harvest { new_age: 2, berries: 3 });
    assert_eq!(r.asked, vec![3]);
    let mut r = Script::new(&[0], &[]);
    assert_eq!(harvest(3, &mut r).unwrap().berries, 1);
    for age in 0..3 {
        let mut r = Script::new(&[], &[]);
        assert!(harvest(age, &mut r).is_none(), "age {age}");
        assert!(r.asked.is_empty());
    }
}

#[test]
fn placing_needs_the_dark_and_a_sturdy_top_or_a_grown_bush() {
    let s = Settings::default();
    let sturdy = Support::Block { sturdy_top: true };
    assert!(can_place(&s, Ore::Iron, 0, sturdy));
    assert!(can_place(&s, Ore::Iron, 12, sturdy));
    // getRawBrightness(pos, 0) < 13.
    assert!(!can_place(&s, Ore::Iron, 13, sturdy));
    assert!(!can_place(&s, Ore::Iron, 0, Support::Block { sturdy_top: false }));
    // On another bush: from the second age (age 2 or 3).
    assert!(!can_place(&s, Ore::Iron, 0, Support::Bush { age: 1 }));
    assert!(can_place(&s, Ore::Iron, 0, Support::Bush { age: 2 }));
    assert!(can_place(&s, Ore::Gold, 0, Support::Bush { age: 3 }));
    // darknessOnly off: any light.
    let mut s2 = Settings::default();
    s2.darkness_only[Ore::Iron.index()] = false;
    assert!(can_place(&s2, Ore::Iron, 15, sturdy));
    assert!(!can_place(&s2, Ore::Gold, 15, sturdy));
}

#[test]
fn shapes_grow_with_the_age() {
    // SHAPE_BY_AGE: box(4,0,4 .. 12,8,12), box(2,0,2 .. 14,12,14), then the whole block.
    assert_eq!(shape(0), [0.25, 0.0, 0.25, 0.75, 0.5, 0.75]);
    assert_eq!(shape(1), [0.125, 0.0, 0.125, 0.875, 0.75, 0.875]);
    assert_eq!(shape(2), [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(shape(3), [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
}

#[test]
fn a_grown_bush_collides_like_a_whole_block() {
    // Ages 2 and 3 are the whole block in SHAPE_BY_AGE; the small ones borrow a close shape.
    assert_eq!(collision_like(3), "minecraft:stone");
    assert_eq!(collision_like(2), "minecraft:stone");
    assert_eq!(collision_like(1), "minecraft:chest");
    assert_eq!(collision_like(0), "minecraft:turtle_egg");
}

#[test]
fn the_cells_an_entity_touches_leave_out_the_ones_it_only_borders() {
    // A player 0.6 wide, 1.8 tall, standing at x 10.5, z 20.5, y 64.
    let min = [10.2, 64.0, 20.2];
    let max = [10.8, 65.8, 20.8];
    assert_eq!(cells_touched(min, max), vec![[10, 64, 20], [10, 65, 20]]);
    // Standing exactly on a corner: four columns.
    let cells = cells_touched([9.7, 64.0, 19.7], [10.3, 65.8, 20.3]);
    assert_eq!(cells.len(), 2 * 2 * 2);
    // A box that ends on the cell's edge does not enter the next one.
    assert_eq!(cells_touched([10.0, 64.0, 20.0], [11.0, 65.0, 21.0]), vec![[10, 64, 20]]);
    assert_eq!(HURT_SOURCE, "minecraft:cactus");
    assert_eq!(HURT_AMOUNT, 1.0);
}

#[test]
fn the_template_classes_that_would_give_a_bush_vanillas_behaviour_become_plain() {
    assert_eq!(inert_class("SweetBerryBushBlock"), Some("Block"));
    assert_eq!(inert_class("BushBlock"), Some("Block"));
    assert_eq!(inert_class("CauldronBlock"), Some("Block"));
    assert_eq!(inert_class("AbstractCauldronBlock"), Some("Block"));
    assert_eq!(inert_class("BonemealableBlock"), Some("Inert"));
    assert_eq!(inert_class("Block"), None);
    assert_eq!(inert_class("FlowerPotBlock"), None);
}

// ------------------------------------------------------------ world generation

/// A world of cells in a box, everything outside it void air that cannot be written.
struct Fake {
    cells: HashMap<Pos, Cell>,
    fill: Cell,
    min: Pos,
    max: Pos,
    bushes: Vec<(Pos, Ore)>,
    /// Cave air at y = -63 + 4k below this height (none when `i32::MIN`).
    layers_below: i32,
}
impl Fake {
    fn new(fill: Cell) -> Self {
        Self {
            cells: HashMap::new(),
            fill,
            min: [-64, -64, -64],
            max: [64, 64, 64],
            bushes: Vec::new(),
            layers_below: i32::MIN,
        }
    }
    fn inside(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.min[i] && p[i] <= self.max[i])
    }
}
impl GenWorld for Fake {
    fn cell(&self, p: Pos) -> Cell {
        if !self.inside(p) {
            return Cell::Air;
        }
        if let Some(c) = self.cells.get(&p) {
            return *c;
        }
        if p[1] >= -63 && p[1] < self.layers_below && (p[1] + 63) % 4 == 0 {
            return Cell::CaveAir;
        }
        self.fill
    }
    fn set_bush(&mut self, p: Pos, ore: Ore) {
        if self.inside(p) {
            self.cells.insert(p, Cell::Other);
            self.bushes.push((p, ore));
        }
    }
}

#[test]
#[ignore = "needs the original's worldgen JSON, which is not published"]
fn the_features_follow_the_originals_files() {
    // Every number of OreBerryFeatures/OreBerryPlacements against the 1.20.1 jar's JSON.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("original/worldgen");
    let number = |text: &str, key: &str| -> i32 {
        let at = text.find(&format!("\"{key}\"")).unwrap_or_else(|| panic!("no {key}"));
        let rest = &text[at + key.len() + 2..];
        let rest = rest.trim_start_matches([':', ' ']);
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap();
        rest[..end].parse().unwrap()
    };
    for ore in ORES {
        let name = ore.bush();
        let file = name.strip_prefix("oreberriesreplanted:").unwrap();
        let configured = std::fs::read_to_string(root.join("configured_feature").join(format!("{file}.json"))).unwrap();
        let placed = std::fs::read_to_string(root.join("placed_feature").join(format!("{file}.json"))).unwrap();
        let f = feature(ore);
        assert_eq!(f.chance, number(&configured, "chance"), "{file} chance");
        assert_eq!(f.bottom_offset, number(&placed, "bottom_offset"), "{file} bottom");
        assert_eq!(f.top_offset, number(&placed, "top_offset"), "{file} top");
        assert_eq!(f.maximum, number(&placed, "maximum"), "{file} maximum");
        assert_eq!(f.rarity, number(&placed, "rarity"), "{file} rarity");
        assert!(configured.contains("\"age\": \"3\""), "{file} is placed grown");
    }
}

#[test]
fn a_feature_origin_is_one_chunk_in_rarity_with_the_height_and_the_column_drawn_in_order() {
    // Iron: rarity 5, y = nextInt(64) - 64, then x and z in the chunk.
    let mut r = Script::new(&[0, 10, 3, 14], &[]);
    let at = feature_origin(&mut r, Ore::Iron, 2, -3).unwrap();
    assert_eq!(r.asked, vec![5, 64, 16, 16]);
    assert_eq!(at, [2 * 16 + 3, 10 - 64, -3 * 16 + 14]);
    // Any other draw of the rarity: nothing (and nothing else is drawn).
    let mut r = Script::new(&[1], &[]);
    assert!(feature_origin(&mut r, Ore::Iron, 0, 0).is_none());
    assert_eq!(r.asked, vec![5]);
    // Copper: -44 + nextInt(60).
    let mut r = Script::new(&[0, 59, 0, 0], &[]);
    assert_eq!(feature_origin(&mut r, Ore::Copper, 0, 0).unwrap()[1], 15);
}

#[test]
fn the_ranges_of_height() {
    // The highest and lowest block a bush can be aimed at: maximum - 1 + bottom_offset.
    let top = |o: Ore| feature(o).bottom_offset + feature(o).maximum - 1;
    assert_eq!(top(Ore::Iron), -1);
    assert_eq!(top(Ore::Gold), -33);
    assert_eq!(top(Ore::Copper), 15);
    assert_eq!(top(Ore::Nickel), 55);
    assert_eq!(top(Ore::Zinc), 5);
}

#[test]
fn the_adequate_location_is_the_ceiling_above_or_else_the_floor_below() {
    let mut w = Fake::new(Cell::BaseStone);
    // A cave pocket at y 10..=12 around (0, *, 0).
    for y in 10..=12 {
        w.cells.insert([0, y, 0], Cell::CaveAir);
    }
    // From inside the pocket, upwards: empty at y 10, y 11 is empty too (not solid); at y 12
    // the block above (y 13) is stone: the stone at 13 is the answer.
    assert_eq!(find_adequate_location(&w, [0, 10, 0]), Some([0, 13, 0]));
    // From stone below the pocket: the upward scan reaches y 10 (empty, y 11 empty), y 11, then
    // y 12 (stone above): same answer, 13.
    assert_eq!(find_adequate_location(&w, [0, 5, 0]), Some([0, 13, 0]));
    // No pocket within 16 above or below: nothing.
    let solid = Fake::new(Cell::BaseStone);
    assert_eq!(find_adequate_location(&solid, [0, 0, 0]), None);
    // An open column under a roof far above: the downward scan finds the floor.
    let mut w = Fake::new(Cell::Air);
    for x in -2..=2 {
        for z in -2..=2 {
            w.cells.insert([x, 0, z], Cell::BaseStone);
        }
    }
    // Upwards the air never ends below the build height limit 64 (void air above it): 16 steps
    // from y 40 find air with air above: no. Then downwards from y 10: y 10 (air), y 9... until
    // y 1 (air) with stone (y 0) below: the floor.
    assert_eq!(find_adequate_location(&w, [0, 10, 0]), Some([0, 0, 0]));
}

fn pocket_world() -> Fake {
    // Stone all around, a 3x3x3 cave of cave air centred on (0, 10, 0).
    let mut w = Fake::new(Cell::BaseStone);
    for x in -1..=1 {
        for y in 9..=11 {
            for z in -1..=1 {
                w.cells.insert([x, y, z], Cell::CaveAir);
            }
        }
    }
    w
}

#[test]
fn a_tiny_node_is_the_block_and_a_few_neighbours() {
    let mut w = pocket_world();
    let at = [0, 12, 0]; // the ceiling stone
    // chance 12, draw 0 (<5) is a tiny node. Then east/west/south/north draws: 0 = yes,
    // and the "above" draw 15 (not < 7) = no.
    let s = Settings::default();
    let mut r = Script::new(&[0, 0, 1, 2, 3, 15], &[]);
    // find_adequate_location is made first (no draws); the origin is inside the pocket.
    place_feature(&mut w, &mut r, &s, Ore::Iron, [0, 9, 0]);
    let _ = at;
    assert_eq!(r.asked, vec![12, 4, 4, 4, 4, 16]);
    // The ceiling block (0,12,0) is stone with cave air below: a bush. East (1,12,0) is stone with
    // cave air below (the pocket is only 3x3): a bush. West, south, north: draws 1,2,3 are not 0.
    let mut got: Vec<Pos> = w.bushes.iter().map(|b| b.0).collect();
    got.sort();
    assert_eq!(got, vec![[0, 12, 0], [1, 12, 0]]);
    assert!(w.bushes.iter().all(|b| b.1 == Ore::Iron));
}

#[test]
fn a_small_node_uses_booleans_for_the_four_sides() {
    let mut w = pocket_world();
    let s = Settings::default();
    // chance 12, draw 7 (>= 5, != 11): small. Booleans east/west/south/north = T,F,T,F; above draw 1 (!= 0): yes.
    let mut r = Script::new(&[7, 1], &[true, false, true, false]);
    place_feature(&mut w, &mut r, &s, Ore::Iron, [0, 9, 0]);
    assert_eq!(r.asked, vec![12, 4]);
    assert_eq!(r.asked_bools, 4);
    let mut got: Vec<Pos> = w.bushes.iter().map(|b| b.0).collect();
    got.sort();
    // (0,12,0) ceiling; east (1,12,0): stone over cave air -> bush; south (0,12,1): bush. Above (0,13,0): the
    // original asks only that the block above OR the one below is not empty, and this one has stone above:
    // it becomes a bush too (the nodes grow into the rock).
    assert_eq!(got, vec![[0, 12, 0], [0, 12, 1], [0, 13, 0], [1, 12, 0]]);
}

#[test]
fn a_medium_node_is_a_scatter_of_a_cube_and_then_a_small_node() {
    let mut w = pocket_world();
    let s = Settings::default();
    // chance 12, draw 11: medium. 27 draws of nextInt(4): yes (0) only for the first and the last;
    // then the small node: 4 booleans (all no) and an "above" draw 0 (no).
    let mut ints = vec![11];
    for i in 0..27 {
        ints.push(if i == 0 || i == 26 { 0 } else { 1 });
    }
    ints.push(0);
    let mut r = Script::new(&ints, &[false, false, false, false]);
    place_feature(&mut w, &mut r, &s, Ore::Iron, [0, 9, 0]);
    assert_eq!(r.asked.iter().filter(|b| **b == 4).count(), 28);
    // The cube is around (0,12,0): offset (-1,-1,-1) = (-1,11,-1) is cave air with stone above: a bush;
    // offset (1,1,1) = (1,13,1) is stone with stone above: a bush. Then the small node puts the centre.
    let mut got: Vec<Pos> = w.bushes.iter().map(|b| b.0).collect();
    got.sort();
    assert_eq!(got, vec![[-1, 11, -1], [0, 12, 0], [1, 13, 1]]);
}

#[test]
fn density_repeats_the_tries_from_the_same_origin() {
    // Copper has density 2: two nodes, each with its own chance draw.
    let mut w = pocket_world();
    let s = Settings::default();
    let mut r = Script::new(&[0, 3, 3, 3, 3, 15, 0, 3, 3, 3, 3, 15], &[]);
    place_feature(&mut w, &mut r, &s, Ore::Copper, [0, 9, 0]);
    assert_eq!(r.asked.iter().filter(|b| **b == 12).count(), 2);
    // The second node's centre is the same stone, now a bush (not cave air or base stone any more).
    assert_eq!(w.bushes.len(), 1);
}

#[test]
fn bushes_replace_base_stone_or_cave_air_and_nothing_else() {
    let mut w = pocket_world();
    // The ceiling block is a block the tag does not hold: no bush there.
    w.cells.insert([0, 12, 0], Cell::Other);
    let s = Settings::default();
    let mut r = Script::new(&[0, 3, 3, 3, 3, 15], &[]);
    place_feature(&mut w, &mut r, &s, Ore::Iron, [0, 9, 0]);
    assert!(w.bushes.is_empty());
    // A cave air block next to solid ground does get one.
    let mut w = pocket_world();
    let mut r = Script::new(&[0, 3, 3, 3, 3, 15], &[]);
    place_feature(&mut w, &mut r, &s, Ore::Iron, [0, 9, 0]);
    assert_eq!(w.bushes.len(), 1);
    assert_eq!(w.cell([0, 12, 0]), Cell::Other);
}

#[test]
fn base_stone_is_the_overworld_tag() {
    for n in ["minecraft:stone", "minecraft:granite", "minecraft:diorite", "minecraft:andesite", "minecraft:tuff", "minecraft:deepslate"] {
        assert!(is_base_stone(n), "{n}");
    }
    for n in ["minecraft:netherrack", "minecraft:dirt", "minecraft:iron_ore", "minecraft:cobblestone"] {
        assert!(!is_base_stone(n), "{n}");
    }
}

/// A world of stone with a cave layer of cave air every four blocks.
fn layered(ceiling: i32) -> Fake {
    let mut w = Fake::new(Cell::BaseStone);
    w.min = [-24 * 16, -64, -24 * 16];
    w.max = [24 * 16, ceiling, 24 * 16];
    w.layers_below = ceiling;
    w
}

#[test]
fn many_chunks_give_each_ore_its_share_inside_its_height_range() {
    // Chunks (-6..6) x (-6..6), a cave layer every four blocks everywhere. Each ore must show up, between
    // its own limits (the origin in [bottom, bottom + maximum), the nodes spreading a little further).
    let s = Settings::default();
    let mut w = layered(120);
    for cx in -6..6 {
        for cz in -6..6 {
            decorate_chunk(&mut w, &s, cx, cz);
        }
    }
    let mut count: HashMap<Ore, usize> = HashMap::new();
    for (p, o) in &w.bushes {
        *count.entry(*o).or_default() += 1;
        let f = feature(*o);
        // A node reaches at most 16 blocks from the origin up or down, plus its size (2).
        assert!(p[1] >= f.bottom_offset - 18, "{o:?} at {p:?}");
        assert!(p[1] <= f.bottom_offset + f.maximum + 18, "{o:?} at {p:?}");
    }
    for o in ORES {
        assert!(count.get(&o).copied().unwrap_or(0) > 0, "{o:?} never appears");
    }
}

#[test]
fn the_same_chunk_always_gets_the_same_bushes() {
    let s = Settings::default();
    let run = || {
        let mut w = layered(120);
        for cx in 0..4 {
            for cz in 0..4 {
                decorate_chunk(&mut w, &s, cx, cz);
            }
        }
        w.bushes
    };
    assert_eq!(run(), run());
}

#[test]
fn the_generator_draws_evenly() {
    let mut p = Prng::for_feature(3, -7, Ore::Zinc);
    let mut hits = [0u32; 16];
    for _ in 0..16000 {
        hits[p.next_int_bound(16) as usize] += 1;
    }
    assert!(hits.iter().all(|h| (800..1200).contains(h)), "{hits:?}");
    let mut p = Prng::for_feature(3, -7, Ore::Zinc);
    let yes = (0..10000).filter(|_| p.next_boolean()).count();
    assert!((4700..5300).contains(&yes), "{yes}");
    // Different chunks and different ores start differently.
    assert_ne!(Prng::for_feature(0, 0, Ore::Iron).0, Prng::for_feature(0, 1, Ore::Iron).0);
    assert_ne!(Prng::for_feature(0, 0, Ore::Iron).0, Prng::for_feature(0, 0, Ore::Gold).0);
}

// ---------------------------------------------------------------- the recipes

#[test]
fn smelting_results_are_the_originals_nugget_tags() {
    assert_eq!(nugget_result(Ore::Iron), Some("minecraft:iron_nugget"));
    assert_eq!(nugget_result(Ore::Gold), Some("minecraft:gold_nugget"));
    assert_eq!(nugget_result(Ore::Copper), Some("minecraft:copper_nugget"));
    // The other metals have no nugget in the game: no recipe (the original's condition).
    for o in [Ore::Tin, Ore::Aluminum, Ore::Lead, Ore::Nickel, Ore::Uranium, Ore::Osmium, Ore::Zinc, Ore::Silver, Ore::Essence] {
        assert_eq!(nugget_result(o), None, "{o:?}");
    }
}

// ------------------------------------------------- the carried content agrees

fn carry() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/mods/oreberriesreplanted/carry")
}
fn names(dir: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter_map(|n| n.strip_suffix(".json").map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[test]
fn the_carried_blocks_and_items_are_the_mods() {
    let root = carry().join("data/oreberriesreplanted/horizon");
    if !root.exists() {
        eprintln!("no carry folder here, skipped");
        return;
    }
    let mut blocks: Vec<String> = ORES.iter().map(|o| o.bush().replace("oreberriesreplanted:", "")).collect();
    blocks.extend(ORES.iter().map(|o| format!("potted_{}", o.bush().replace("oreberriesreplanted:", ""))));
    blocks.extend(["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "crimson", "warped"].map(|w| format!("{w}_vat")));
    blocks.sort();
    assert_eq!(names(&root.join("block")), blocks);
    let read = |kind: &str, n: &str| std::fs::read_to_string(root.join(kind).join(format!("{n}.json"))).unwrap();
    for ore in ORES {
        let n = ore.bush().replace("oreberriesreplanted:", "");
        let t = read("block", &n);
        assert!(t.contains("\"template\": \"minecraft:sweet_berry_bush\""), "{n}");
        assert!(t.contains("\"hardness\": 0.3") && t.contains("\"explosion_resistance\": 0.3"), "{n}");
        assert!(read("block", &format!("potted_{n}")).contains("minecraft:potted_fern"));
    }
    let mut items: Vec<String> = ORES
        .iter()
        .filter(|o| **o != Ore::Essence)
        .flat_map(|o| [o.berry().replace("oreberriesreplanted:", ""), format!("{}_juice_bucket", o.berry().replace("oreberriesreplanted:", ""))])
        .collect();
    items.push("essence_berry".into());
    items.push("copper_nugget".into());
    items.sort();
    assert_eq!(names(&root.join("item")), items);
    // The recipes: a smelting and a blasting for each metal that has a nugget, with the originals' times.
    let recipes = carry().join("data/oreberriesreplanted/recipe");
    for ore in ORES {
        for (kind, time) in [("smelting", 200), ("blasting", 100)] {
            let file = recipes.join(format!("{}_from_{kind}.json", ore.name()));
            let Some(result) = nugget_result(ore) else {
                assert!(!file.exists(), "{ore:?} {kind} has no nugget to give");
                continue;
            };
            let t = std::fs::read_to_string(file).unwrap();
            assert!(t.contains(&format!("\"type\": \"minecraft:{kind}\"")), "{ore:?} {kind}");
            assert!(t.contains(&format!("\"cookingtime\": {time}")), "{ore:?} {kind}");
            assert!(t.contains("\"experience\": 0.2"), "{ore:?} {kind}");
            assert!(t.contains(&format!("\"ingredient\": \"{}\"", ore.berry())), "{ore:?} {kind}");
            assert!(t.contains(&format!("\"id\": \"{result}\"")), "{ore:?} {kind}");
        }
    }
}
