use horizon_torchmaster::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn s() -> Settings {
    Settings::default()
}
fn none() -> Filters {
    Filters::new(&s(), &|_| true)
}
const ZOMBIE: (&str, Category) = ("minecraft:zombie", Category::Monster);
const COW: (&str, Category) = ("minecraft:cow", Category::Creature);

fn block(r: &Registry, who: (&str, Category), reason: Reason, at: [f64; 3]) -> bool {
    r.should_block_spawn(&s(), &none(), who.0, who.1, reason, at)
}

#[test]
fn settings_are_tuned() {
    // config/torchmaster.toml of the the 1.20.1 pack profile.
    let s = s();
    assert_eq!((s.mega_torch_radius, s.dread_lamp_radius), (64, 64));
    assert!(s.block_only_natural_spawns && s.block_village_sieges && s.beginner_tooltips);
    assert_eq!(
        (
            s.feral_flare_radius,
            s.feral_flare_tick_rate,
            s.feral_flare_min_light_level,
            s.feral_flare_light_hardcap
        ),
        (16, 5, 10, 255)
    );
    assert!(s.mega_torch_overrides.is_empty() && s.dread_lamp_overrides.is_empty());
}

#[test]
fn cube_reaches_radius_below_and_radius_plus_one_above() {
    let t = [100, 64, -40];
    for axis in 0..3 {
        let at = |d: i32| {
            let mut p = t;
            p[axis] += d;
            p
        };
        assert!(in_cube(at(-64), t, 64));
        assert!(!in_cube(at(-65), t, 64));
        assert!(in_cube(at(65), t, 64));
        assert!(!in_cube(at(66), t, 64));
    }
    assert!(in_cube([36, 0, 25], t, 64));
    assert!(!in_cube([35, 0, 25], t, 64));
    // Radius 0 is the torch and the blocks one above on each axis.
    assert!(in_cube(t, t, 0) && in_cube([101, 65, -39], t, 0) && !in_cube([99, 64, -40], t, 0));
}

#[test]
fn spawn_position_is_truncated_like_a_java_int_cast() {
    assert_eq!(block_pos([-0.5, 64.0, 10.9]), [0, 64, 10]);
    assert_eq!(block_pos([-63.5, -1.5, 2.5]), [-63, -1, 2]);
    assert_eq!(block_pos([f64::NAN, 1e12, -1e12]), [0, i32::MAX, i32::MIN]);
    // A mob at the centre of block x = -65 counts as x = -64: inside a torch at 0.
    let mut r = Registry::default();
    r.register([0, 64, 0], Light::MegaTorch);
    assert!(block(&r, ZOMBIE, Reason::Natural, [-64.5, 64.0, 0.5]));
    assert!(!block(&r, ZOMBIE, Reason::Natural, [-65.5, 64.0, 0.5]));
    // On the positive side the centre of x = 65 is in, 66 out.
    assert!(block(&r, ZOMBIE, Reason::Natural, [65.5, 64.0, 0.5]));
    assert!(!block(&r, ZOMBIE, Reason::Natural, [66.5, 64.0, 0.5]));
}

#[test]
fn mega_torch_blocks_monsters_and_dread_lamp_the_rest_but_misc() {
    let f = none();
    let all = [
        Category::Monster,
        Category::Creature,
        Category::Ambient,
        Category::Axolotls,
        Category::UndergroundWaterCreature,
        Category::WaterCreature,
        Category::WaterAmbient,
        Category::Misc,
    ];
    for c in all {
        assert_eq!(f.blocks(Light::MegaTorch, "x:y", c), c == Category::Monster, "{c:?}");
        assert_eq!(
            f.blocks(Light::DreadLamp, "x:y", c),
            c != Category::Monster && c != Category::Misc,
            "{c:?}"
        );
    }
    let mut r = Registry::default();
    r.register([0, 64, 0], Light::MegaTorch);
    assert!(block(&r, ZOMBIE, Reason::Natural, [10.5, 64.0, 10.5]));
    assert!(!block(&r, COW, Reason::Natural, [10.5, 64.0, 10.5]));
    let mut d = Registry::default();
    d.register([0, 64, 0], Light::DreadLamp);
    assert!(block(&d, COW, Reason::Natural, [10.5, 64.0, 10.5]));
    assert!(block(&d, ("minecraft:bat", Category::Ambient), Reason::Natural, [0.5, 30.0, 0.5]));
    assert!(block(&d, ("minecraft:squid", Category::WaterCreature), Reason::Natural, [0.5, 50.0, 0.5]));
    assert!(!block(&d, ZOMBIE, Reason::Natural, [10.5, 64.0, 10.5]));
    assert_eq!(Category::from_name("water_ambient"), Category::WaterAmbient);
    assert_eq!(Category::from_name("unknown"), Category::Misc);
}

#[test]
fn only_natural_chunk_generation_and_patrol_spawns_are_blocked() {
    let mut r = Registry::default();
    r.register([0, 64, 0], Light::MegaTorch);
    r.register([0, 64, 0], Light::MegaTorch);
    let at = [3.5, 64.0, 3.5];
    use Reason::*;
    for reason in [Natural, ChunkGeneration, Patrol] {
        assert!(block(&r, ZOMBIE, reason, at), "{reason:?}");
    }
    for reason in [
        Spawner, Structure, Breeding, MobSummoned, Jockey, Event, Conversion, Reinforcement,
        Triggered, Bucket, SpawnEgg, Command, Dispenser,
    ] {
        assert!(!block(&r, ZOMBIE, reason, at), "{reason:?}");
    }
    // blockOnlyNaturalSpawns = false also blocks spawners, never intentional spawns.
    let mut all = s();
    all.block_only_natural_spawns = false;
    let f = none();
    assert!(r.should_block_spawn(&all, &f, ZOMBIE.0, ZOMBIE.1, Spawner, at));
    assert!(r.should_block_spawn(&all, &f, ZOMBIE.0, ZOMBIE.1, Structure, at));
    assert!(!r.should_block_spawn(&all, &f, ZOMBIE.0, ZOMBIE.1, SpawnEgg, at));
    assert!(!r.should_block_spawn(&all, &f, ZOMBIE.0, ZOMBIE.1, Event, at));
}

#[test]
fn several_torches_and_a_removed_one() {
    let mut r = Registry::default();
    assert!(r.block_changed([0, 64, 0], Kind::Other, Kind::MegaTorch).is_empty());
    assert!(r.block_changed([300, 64, 0], Kind::Other, Kind::MegaTorch).is_empty());
    let between = [150.5, 64.0, 0.5];
    let near_second = [280.5, 64.0, 0.5];
    assert!(!block(&r, ZOMBIE, Reason::Natural, between));
    assert!(block(&r, ZOMBIE, Reason::Natural, near_second));
    assert!(block(&r, ZOMBIE, Reason::Natural, [20.5, 64.0, 0.5]));
    // The second torch broken: onRemove unregisters it at once.
    r.block_changed([300, 64, 0], Kind::MegaTorch, Kind::Other);
    assert!(!block(&r, ZOMBIE, Reason::Natural, near_second));
    assert!(block(&r, ZOMBIE, Reason::Natural, [20.5, 64.0, 0.5]));
    // A dread lamp in the first one's place: monsters spawn again, cows don't.
    r.block_changed([0, 64, 0], Kind::MegaTorch, Kind::DreadLamp);
    assert!(!block(&r, ZOMBIE, Reason::Natural, [20.5, 64.0, 0.5]));
    assert!(block(&r, COW, Reason::Natural, [20.5, 64.0, 0.5]));
    assert_eq!(r.lights.len(), 1);
    // Two lights overlapping: both answer, either one blocks.
    r.block_changed([10, 64, 0], Kind::Other, Kind::MegaTorch);
    assert!(block(&r, ZOMBIE, Reason::Natural, [70.5, 64.0, 0.5]));
    assert!(block(&r, COW, Reason::Natural, [60.5, 64.0, 0.5]));
    // A state change of the same block (a rod turned up) keeps it.
    r.block_changed([10, 64, 0], Kind::MegaTorch, Kind::MegaTorch);
    assert_eq!(r.lights.get(&[10, 64, 0]), Some(&Light::MegaTorch));
}

#[test]
fn block_list_overrides() {
    let exists = |id: &str| id != "minecraft:nonexistent";
    let mut s = s();
    s.mega_torch_overrides = vec!["-minecraft:creeper".into(), "+cow".into()];
    s.dread_lamp_overrides = vec![
        "+minecraft:zombie".into(),
        "-minecraft:pig".into(),
        "+minecraft:nonexistent".into(),
        "+a".into(),
        "*minecraft:cow".into(),
    ];
    let f = Filters::new(&s, &exists);
    assert!(!f.blocks(Light::MegaTorch, "minecraft:creeper", Category::Monster));
    assert!(f.blocks(Light::MegaTorch, "minecraft:skeleton", Category::Monster));
    assert!(f.blocks(Light::MegaTorch, "minecraft:cow", Category::Creature));
    assert!(f.blocks(Light::DreadLamp, "minecraft:zombie", Category::Monster));
    assert!(!f.blocks(Light::DreadLamp, "minecraft:pig", Category::Creature));
    assert!(f.blocks(Light::DreadLamp, "minecraft:cow", Category::Creature));
    assert!(!f.blocks(Light::DreadLamp, "minecraft:nonexistent", Category::Misc));
    // The last of a + and a - wins.
    s.mega_torch_overrides = vec!["-minecraft:zombie".into(), "+minecraft:zombie".into()];
    assert!(Filters::new(&s, &exists).blocks(Light::MegaTorch, "minecraft:zombie", Category::Monster));
    s.mega_torch_overrides = vec!["+minecraft:cow".into(), "-minecraft:cow".into()];
    assert!(!Filters::new(&s, &exists).blocks(Light::MegaTorch, "minecraft:cow", Category::Creature));
}

#[test]
fn village_sieges_only_near_mega_torches() {
    let mut r = Registry::default();
    r.register([0, 64, 0], Light::DreadLamp);
    assert!(!r.should_block_siege(&s(), [10.5, 64.0, 10.5]));
    r.register([100, 64, 0], Light::MegaTorch);
    assert!(r.should_block_siege(&s(), [40.5, 64.0, 10.5]));
    assert!(!r.should_block_siege(&s(), [34.5, 64.0, 10.5]));
    let mut off = s();
    off.block_village_sieges = false;
    assert!(!r.should_block_siege(&off, [40.5, 64.0, 10.5]));
}

/// A world of kinds, with the chunks (x >> 4, z >> 4) that are loaded.
#[derive(Default)]
struct Fake {
    blocks: BTreeMap<Pos, Kind>,
    loaded: BTreeSet<(i32, i32)>,
    light: BTreeMap<Pos, i32>,
    surface: i32,
    random: VecDeque<i32>,
    removed: Vec<Pos>,
    placed: Vec<Pos>,
}
impl Fake {
    fn all_loaded() -> Self {
        let mut f = Fake {
            surface: 70,
            ..Default::default()
        };
        for x in -8..8 {
            for z in -8..8 {
                f.loaded.insert((x, z));
            }
        }
        f
    }
    fn is_loaded(&self, p: Pos) -> bool {
        (-64..320).contains(&p[1]) && self.loaded.contains(&(p[0] >> 4, p[2] >> 4))
    }
}
impl Blocks for Fake {
    fn kind(&self, p: Pos) -> Option<Kind> {
        self.is_loaded(p)
            .then(|| self.blocks.get(&p).copied().unwrap_or(Kind::Other))
    }
}
impl LightEditor for Fake {
    fn remove_light(&mut self, p: Pos) {
        self.blocks.remove(&p);
        self.removed.push(p);
    }
}
impl LanternWorld for Fake {
    fn loaded(&self, p: Pos) -> bool {
        self.is_loaded(p)
    }
    fn surface(&self, _: i32, _: i32) -> i32 {
        self.surface
    }
    fn height(&self) -> i32 {
        384
    }
    fn empty(&self, p: Pos) -> bool {
        // Positions not set are air here; Other stands for a solid block.
        matches!(self.blocks.get(&p), None | Some(Kind::Light))
    }
    fn block_light(&self, p: Pos) -> i32 {
        if self.blocks.get(&p) == Some(&Kind::Light) {
            return 15;
        }
        self.light.get(&p).copied().unwrap_or(0)
    }
    fn place_light(&mut self, p: Pos) -> bool {
        if self.blocks.get(&p) == Some(&Kind::Light) {
            return false;
        }
        self.blocks.insert(p, Kind::Light);
        self.placed.push(p);
        true
    }
    fn next_int(&mut self, bound: i32) -> i32 {
        let v = self.random.pop_front().unwrap_or(0);
        assert!((0..bound).contains(&v), "{v} not below {bound}");
        v
    }
}

#[test]
fn registry_cleanup_runs_every_201_ticks_on_loaded_chunks_only() {
    let mut w = Fake::all_loaded();
    let mut r = Registry::default();
    r.register([0, 64, 0], Light::MegaTorch); // gone, loaded
    r.register([5000, 64, 0], Light::MegaTorch); // gone, not loaded
    r.register([16, 64, 16], Light::DreadLamp);
    w.blocks.insert([16, 64, 16], Kind::DreadLamp);
    for _ in 0..200 {
        assert!(!r.global_tick(&w));
    }
    assert_eq!(r.lights.len(), 3);
    assert!(r.global_tick(&w));
    assert_eq!(r.lights.keys().copied().collect::<Vec<_>>(), [[16, 64, 16], [5000, 64, 0]]);
    // The counter starts over.
    for _ in 0..200 {
        assert!(!r.global_tick(&w));
    }
    assert!(r.global_tick(&w));
}

#[test]
fn lantern_places_lights_every_fifth_tick_in_the_dark() {
    let mut w = Fake::all_loaded();
    let s = s();
    let at = [0, 64, 0];
    let mut l = Lantern::default();
    // nextInt(32) three times: offsets 16 - v, so v = 0 is +16 and 31 is -15.
    w.random = VecDeque::from([0, 16, 31]);
    for _ in 0..4 {
        assert_eq!(tick_lantern(at, &mut l, &s, &mut w), None);
    }
    assert!(w.random.len() == 3, "no draws before the fifth tick");
    assert_eq!(tick_lantern(at, &mut l, &s, &mut w), Some([16, 64, -15]));
    assert_eq!(l.lights, vec![[16, 64, -15]]);
    assert_eq!(l.ticks, 0);
    // A lit spot (light 10 or more) gets nothing, a dark one does.
    w.light.insert([1, 65, 1], 10);
    w.random = VecDeque::from([15, 15, 15]);
    for _ in 0..5 {
        tick_lantern(at, &mut l, &s, &mut w);
    }
    assert_eq!(l.lights.len(), 1);
    w.light.insert([1, 65, 1], 9);
    w.random = VecDeque::from([15, 15, 15]);
    for _ in 0..5 {
        tick_lantern(at, &mut l, &s, &mut w);
    }
    assert_eq!(l.lights, vec![[16, 64, -15], [1, 65, 1]]);
    // A solid block gets nothing.
    w.blocks.insert([2, 66, 2], Kind::Other);
    w.random = VecDeque::from([14, 14, 14]);
    for _ in 0..5 {
        tick_lantern(at, &mut l, &s, &mut w);
    }
    assert_eq!(l.lights.len(), 2);
}

#[test]
fn lantern_clamps_to_four_above_the_surface_and_skips_unloaded() {
    let mut w = Fake::all_loaded();
    w.surface = 50;
    let s = s();
    let at = [0, 64, 0];
    let mut l = Lantern::default();
    // y offset +16: 80 > 50 + 4, so 54.
    w.random = VecDeque::from([10, 0, 10]);
    for _ in 0..5 {
        tick_lantern(at, &mut l, &s, &mut w);
    }
    assert_eq!(l.lights, vec![[6, 54, 6]]);
    // Below the clamp, the drawn height stays.
    w.surface = 100;
    w.random = VecDeque::from([10, 31, 10]);
    for _ in 0..5 {
        tick_lantern(at, &mut l, &s, &mut w);
    }
    assert_eq!(l.lights[1], [6, 49, 6]);
    // An unloaded target is skipped (the draws are made all the same).
    w.loaded.remove(&(1, 0));
    w.random = VecDeque::from([0, 10, 10]);
    for _ in 0..5 {
        tick_lantern(at, &mut l, &s, &mut w);
    }
    assert!(w.random.is_empty());
    assert_eq!(l.lights.len(), 2);
}

#[test]
fn lantern_stops_one_light_past_the_hardcap() {
    let mut w = Fake::all_loaded();
    let s = s();
    let at = [0, 0, 0];
    let mut l = Lantern::default();
    let mut placed = 0;
    'spots: for x in 0..32 {
        for y in 0..32 {
            for z in 0..32 {
                w.random = VecDeque::from([x, y, z]);
                for _ in 0..5 {
                    if tick_lantern(at, &mut l, &s, &mut w).is_some() {
                        placed += 1;
                    }
                }
                if placed > 300 {
                    break 'spots;
                }
            }
        }
    }
    // `childLights.size() > 255` stops it: 256 lights.
    assert_eq!(placed, 256);
    assert_eq!(l.lights.len(), 256);
}

#[test]
fn breaking_a_lantern_removes_its_lights_and_waits_for_unloaded_ones() {
    let mut w = Fake::all_loaded();
    let mut r = Registry::default();
    let at = [0, 64, 0];
    r.block_changed(at, Kind::Other, Kind::Lantern);
    assert_eq!(r.lanterns.get(&at), Some(&Lantern::default()));
    let lights = vec![[1, 64, 1], [2, 64, 2], [300, 64, 0], [3, 64, 3]];
    for p in &lights {
        w.blocks.insert(*p, Kind::Light);
    }
    // The player built over one of them.
    w.blocks.insert([3, 64, 3], Kind::Other);
    r.lanterns.get_mut(&at).unwrap().lights = lights.clone();
    w.loaded.remove(&(300 >> 4, 0));
    let gone = r.block_changed(at, Kind::Lantern, Kind::Other);
    assert_eq!(gone, lights);
    assert!(r.lanterns.is_empty());
    r.remove_lights(gone, &mut w);
    assert_eq!(w.removed, vec![[1, 64, 1], [2, 64, 2]]);
    assert_eq!(w.blocks.get(&[3, 64, 3]), Some(&Kind::Other));
    assert_eq!(r.orphans.iter().copied().collect::<Vec<_>>(), [[300, 64, 0]]);
    assert!(!r.clear_orphans(&mut w));
    w.loaded.insert((300 >> 4, 0));
    assert!(r.clear_orphans(&mut w));
    assert!(r.orphans.is_empty());
    assert_eq!(w.removed.last(), Some(&[300, 64, 0]));
    // A new lantern in its place starts with no lights.
    r.block_changed(at, Kind::Other, Kind::Lantern);
    assert!(r.lanterns[&at].lights.is_empty());
}

#[test]
fn blocks_found_in_arriving_chunks_are_registered_once() {
    let mut r = Registry::default();
    assert!(r.found([1, 2, 3], Kind::MegaTorch));
    assert!(!r.found([1, 2, 3], Kind::MegaTorch));
    assert!(r.found([1, 2, 3], Kind::DreadLamp));
    assert!(r.found([4, 5, 6], Kind::Lantern));
    r.lanterns.get_mut(&[4, 5, 6]).unwrap().lights.push([5, 5, 5]);
    assert!(!r.found([4, 5, 6], Kind::Lantern));
    assert_eq!(r.lanterns[&[4, 5, 6]].lights, vec![[5, 5, 5]]);
    assert!(!r.found([7, 7, 7], Kind::Light));
    assert!(!r.found([7, 7, 7], Kind::Other));
}

#[test]
fn frozen_pearl_reaches_fifteen_blocks_each_way() {
    let area: Vec<Pos> = frozen_pearl_area([10, 64, -10]).collect();
    assert_eq!(area.len(), 31 * 31 * 31);
    assert_eq!(area.first(), Some(&[-5, 49, -25]));
    assert_eq!(area.last(), Some(&[25, 79, 5]));
    assert_eq!(area[1], [-5, 49, -24]);
}

#[test]
fn tooltips_are_the_description_ids() {
    assert_eq!(tooltip_key("torchmaster:megatorch"), Some("block.torchmaster.megatorch.tooltip"));
    assert_eq!(tooltip_key("torchmaster:frozen_pearl"), Some("item.torchmaster.frozen_pearl.tooltip"));
    assert_eq!(tooltip_key("minecraft:torch"), None);
}

#[test]
fn the_registry_survives_save_and_reload() {
    let mut r = Registry::default();
    r.register([1, -2, 3], Light::MegaTorch);
    r.register([-100, 64, 7], Light::DreadLamp);
    r.lanterns.insert(
        [5, 6, 7],
        Lantern {
            ticks: 3,
            lights: vec![[6, 6, 7], [6, 6, 7], [-200, 54, 7]],
        },
    );
    r.lanterns.insert([9, 9, 9], Lantern::default());
    r.orphans.insert([40, 41, 42]);
    r.cleanup_counter = 77;
    let text = save(&r);
    let back = load(&text);
    assert_eq!(back.lights, r.lights);
    assert_eq!(back.lanterns, r.lanterns);
    assert_eq!(back.orphans, r.orphans);
    // The cleanup counter is not saved (RegistryContainer.tickCounter).
    assert_eq!(back.cleanup_counter, 0);
    // A reloaded torch still blocks and a reloaded lantern still owns its lights.
    assert!(block(&back, ZOMBIE, Reason::Natural, [60.5, 30.0, 3.5]));
    assert!(load("garbage\nlight lamp 1 2 3\nlight megatorch 1 x 3\n").lights.is_empty());
    assert_eq!(load("").lights.len(), 0);
}
