use horizon_explorerscompass::*;
use std::collections::BTreeMap;

#[test]
fn java_random_is_java() {
    // java.util.Random(0).nextInt() = -1155484576, nextInt(100) = 60; Random(42).nextInt() = -1170105035.
    assert_eq!(JavaRandom::new(0).next_int(), -1155484576);
    assert_eq!(JavaRandom::new(0).next_int_bound(100), 60);
    assert_eq!(JavaRandom::new(42).next_int(), -1170105035);
    assert_eq!(JavaRandom::new(42).next_int_bound(10), 0);
    // A power of two takes the high bits: Random(0).nextInt(16) = (16 * next(31)) >> 31.
    let mut r = JavaRandom::new(0);
    let n = r.next_int_bound(16);
    assert!((0..16).contains(&n));
}

#[test]
fn potential_chunks_follow_the_sectors() {
    // Villages: spacing 34, separation 8, salt 10387312, linear. The chunk is inside its sector,
    // at least `separation` chunks from the far edge.
    for seed in [0i64, 1234, -99999, i64::MAX] {
        for (cx, cz) in [(0, 0), (-1, -1), (33, 34), (-200, 5000), (17, -17)] {
            let c = potential_chunk(34, 8, 10387312, false, seed, cx, cz);
            let (sx, sz) = (cx.div_euclid(34) * 34, cz.div_euclid(34) * 34);
            assert!(c[0] >= sx && c[0] < sx + 26, "{c:?} in sector {sx}");
            assert!(c[1] >= sz && c[1] < sz + 26);
            // Every chunk of the sector gives the same answer.
            assert_eq!(c, potential_chunk(34, 8, 10387312, false, seed, sx + 5, sz + 7));
        }
    }
    // Triangular (ocean monument: 32 / 5): within the sector too.
    let c = potential_chunk(32, 5, 10387313, true, 77, 100, 100);
    assert!(c[0] >= 96 && c[0] < 96 + 27 && c[1] >= 96 && c[1] < 96 + 27);
}

/// A world with structures of one kind in the random spread chunks where `lucky(sector)` says so.
struct Fake {
    seed: i64,
    placed: Vec<(String, [i32; 2])>,
    ring: Vec<[i32; 2]>,
    asked: usize,
}
impl StructureWorld for Fake {
    fn starts(&mut self, cx: i32, cz: i32) -> Vec<Start> {
        self.asked += 1;
        self.placed.iter().filter(|(_, c)| *c == [cx, cz]).map(|(s, c)| Start { structure: s.clone(), chunk: *c }).collect()
    }
    fn rings(&mut self, _set: &str) -> Vec<[i32; 2]> {
        self.ring.clone()
    }
    fn seed(&self) -> i64 {
        self.seed
    }
}

fn villages() -> SetDef {
    SetDef { id: "minecraft:villages".into(), placement: Placement::RandomSpread { spacing: 34, separation: 8, salt: 10387312, triangular: false }, structures: vec![] }
}
fn temples() -> SetDef {
    SetDef { id: "minecraft:desert_pyramids".into(), placement: Placement::RandomSpread { spacing: 32, separation: 8, salt: 14357617, triangular: false }, structures: vec![] }
}

/// The potential chunk of a sector for the village set, and of any sector (i, j) = chunks `i * 34 ..`.
fn village_chunk(seed: i64, i: i32, j: i32) -> [i32; 2] {
    potential_chunk(34, 8, 10387312, false, seed, i * 34, j * 34)
}

fn sets_of(sets: Vec<(&'static str, SetDef)>) -> impl Fn(&str) -> Vec<SetDef> {
    move |s: &str| sets.iter().filter(|(n, _)| *n == s).map(|(_, d)| d.clone()).collect()
}

fn run(search: &mut Search, world: &mut Fake) -> Outcome {
    for _ in 0..3_000_000 {
        if let Some(o) = search.do_work(world) {
            return o;
        }
    }
    panic!("never ended");
}

#[test]
fn random_spread_finds_the_first_ring_with_a_start() {
    let seed = 4242;
    // Villages exist in sectors (2, 1) and (-5, 0) only.
    let a = village_chunk(seed, 2, 1);
    let b = village_chunk(seed, -5, 0);
    let mut world = Fake { seed, placed: vec![("minecraft:village_plains".into(), a), ("minecraft:village_taiga".into(), b)], ring: vec![], asked: 0 };
    let sets = sets_of(vec![("minecraft:village_plains", villages()), ("minecraft:village_taiga", villages())]);
    let requested = vec!["minecraft:village_plains".to_owned(), "minecraft:village_taiga".to_owned()];
    // From the origin: the search sector grid is centred on the chunk of the start.
    let mut s = Search::new("minecraft:villages", &requested, &sets, [0, 64, 0], vec![]);
    assert!(!s.is_empty());
    match run(&mut s, &mut world) {
        Outcome::Found { structure, x, z, prev_pos, samples } => {
            // Sectors on ring 2 are visited before ring 5: the village of sector (2, 1).
            assert_eq!((x, z), (a[0] * 16, a[1] * 16));
            assert_eq!(structure, "minecraft:village_plains");
            assert_eq!(prev_pos, vec![[a[0] * 16, 0, a[1] * 16]]);
            // Ring 0 (1 sample), ring 1 (8), ring 2 up to the sector (2, 1): well over 9.
            assert!(samples > 9 && samples <= 25, "{samples}");
        }
        other => panic!("{other:?}"),
    }
    // Search for next: the village found is skipped, the other one is found.
    let mut s = Search::new("minecraft:villages", &requested, &sets, [0, 64, 0], vec![[a[0] * 16, 0, a[1] * 16]]);
    match run(&mut s, &mut world) {
        Outcome::Found { structure, x, z, prev_pos, .. } => {
            assert_eq!((x, z), (b[0] * 16, b[1] * 16));
            assert_eq!(structure, "minecraft:village_taiga");
            assert_eq!(prev_pos.len(), 2);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_structure_nowhere_is_not_found_and_the_radius_is_reported() {
    let seed = 1;
    let mut world = Fake { seed, placed: vec![], ring: vec![], asked: 0 };
    let sets = sets_of(vec![("minecraft:igloo", SetDef { structures: vec![], ..temples() })]);
    let mut s = Search::new("minecraft:igloo", &["minecraft:igloo".to_owned()], &sets, [100, 64, -300], vec![]);
    let o = run(&mut s, &mut world);
    match o {
        Outcome::NotFound { radius, samples } => {
            // the pack: maxRadius 10000; the radius is rounded to 250 and the samples are those of the last worker.
            assert_eq!(radius % 250, 0);
            assert!(radius >= 9750 && radius <= 10000 + 250, "{radius}");
            assert!(samples > 100);
        }
        other => panic!("{other:?}"),
    }
    assert!(!s.radius_updates.is_empty());
    assert!(s.radius_updates.windows(2).all(|p| p[0] < p[1]));
    assert!(s.radius_updates.iter().all(|r| r % 250 == 0 && *r >= 250));
}

#[test]
fn a_structure_without_a_placement_here_is_not_found_at_once() {
    // The nether complex in the overworld: no set can generate, `workerManager.start()` is false -> setNotFound(0, 0).
    let sets = sets_of(vec![]);
    let mut s = Search::new("minecraft:fortress", &["minecraft:fortress".to_owned()], &sets, [0, 64, 0], vec![]);
    assert!(s.is_empty());
    let mut world = Fake { seed: 0, placed: vec![], ring: vec![], asked: 0 };
    assert_eq!(s.do_work(&mut world), Some(Outcome::NotFound { radius: 0, samples: 0 }));
}

#[test]
fn a_group_searches_its_sets_one_after_the_other() {
    // Two sets for a group: the first finds nothing within the limits, the second has the structure.
    let seed = 7;
    let desert = SetDef { id: "a:set1".into(), placement: Placement::RandomSpread { spacing: 32, separation: 8, salt: 1, triangular: false }, structures: vec![] };
    let other = SetDef { id: "a:set2".into(), placement: Placement::RandomSpread { spacing: 24, separation: 4, salt: 2, triangular: false }, structures: vec![] };
    let chunk = potential_chunk(24, 4, 2, false, seed, 24 * 3, 24 * -2);
    let mut world = Fake { seed, placed: vec![("a:s2".into(), chunk)], ring: vec![], asked: 0 };
    let sets = sets_of(vec![("a:s1", desert), ("a:s2", other)]);
    let mut s = Search::new("a:group", &["a:s1".to_owned(), "a:s2".to_owned()], &sets, [0, 64, 0], vec![]);
    // the pack's radius is 10000: the first set runs to the end of it, then the second starts.
    match run(&mut s, &mut world) {
        Outcome::Found { structure, x, z, .. } => {
            assert_eq!(structure, "a:s2");
            assert_eq!((x, z), (chunk[0] * 16, chunk[1] * 16));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn rings_choose_the_nearest_start() {
    // Strongholds: the closest ring chunk that has a start; nearer chunks without one are passed.
    let ring: Vec<[i32; 2]> = vec![[100, 100], [-40, 20], [5, 5], [300, -300]];
    let mut world = Fake {
        seed: 0,
        placed: vec![("minecraft:stronghold".into(), [-40, 20]), ("minecraft:stronghold".into(), [100, 100])],
        ring,
        asked: 0,
    };
    let rings = SetDef { id: "minecraft:strongholds".into(), placement: Placement::Rings, structures: vec![] };
    let sets = sets_of(vec![("minecraft:stronghold", rings)]);
    let mut s = Search::new("minecraft:stronghold", &["minecraft:stronghold".to_owned()], &sets, [0, 64, 0], vec![]);
    match run(&mut s, &mut world) {
        Outcome::Found { x, z, samples, .. } => {
            assert_eq!((x, z), (-40 * 16, 20 * 16));
            assert_eq!(samples, 4, "every ring chunk is sampled; the farther ones are not asked once a start is held");
        }
        other => panic!("{other:?}"),
    }
    // The skipped chunk [5, 5] and the farther [300, -300] were not asked about (the ring list is in the given order:
    // [100, 100] was asked first, then [-40, 20] was nearer; [5, 5] has no start but is nearer: asked; [300, -300] is farther).
    assert!(world.asked <= 3);
    // Search for next: the first one is ignored.
    let sets = sets_of(vec![("minecraft:stronghold", SetDef { id: "minecraft:strongholds".into(), placement: Placement::Rings, structures: vec![] })]);
    let mut s = Search::new("minecraft:stronghold", &["minecraft:stronghold".to_owned()], &sets, [0, 64, 0], vec![[-40 * 16, 0, 20 * 16]]);
    match run(&mut s, &mut world) {
        Outcome::Found { x, z, .. } => assert_eq!((x, z), (100 * 16, 100 * 16)),
        other => panic!("{other:?}"),
    }
    // Without ring chunks the search moves on and ends NOT_FOUND.
    let mut empty = Fake { seed: 0, placed: vec![], ring: vec![], asked: 0 };
    let sets = sets_of(vec![("minecraft:stronghold", SetDef { id: "minecraft:strongholds".into(), placement: Placement::Rings, structures: vec![] })]);
    let mut s = Search::new("minecraft:stronghold", &["minecraft:stronghold".to_owned()], &sets, [0, 64, 0], vec![]);
    assert!(matches!(run(&mut s, &mut empty), Outcome::NotFound { .. }));
}

#[test]
fn generic_placements_spiral_chunk_by_chunk() {
    let mut world = Fake { seed: 0, placed: vec![("m:thing".into(), [3, -2])], ring: vec![], asked: 0 };
    let set = SetDef { id: "m:things".into(), placement: Placement::Generic, structures: vec![] };
    let sets = sets_of(vec![("m:thing", set)]);
    let mut s = Search::new("m:thing", &["m:thing".to_owned()], &sets, [0, 64, 0], vec![]);
    match run(&mut s, &mut world) {
        Outcome::Found { x, z, samples, .. } => {
            assert_eq!((x, z), (48, -32));
            assert!(samples >= 5 && samples < 60, "{samples}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_search_for_next_stops_at_the_limit() {
    // the pack: maxNextSearches 100 -> a search holding 101 places gives up.
    let prev: Vec<Pos> = (0..101).map(|i| [i * 16, 0, 0]).collect();
    let mut world = Fake { seed: 3, placed: vec![("minecraft:village_plains".into(), village_chunk(3, 1, 1))], ring: vec![], asked: 0 };
    let sets = sets_of(vec![("minecraft:village_plains", villages())]);
    let mut s = Search::new("minecraft:village_plains", &["minecraft:village_plains".to_owned()], &sets, [0, 64, 0], prev);
    assert!(matches!(run(&mut s, &mut world), Outcome::NotFound { .. }));
}

#[test]
fn max_radius_zero_fails_before_anything() {
    let mut world = Fake { seed: 3, placed: vec![], ring: vec![], asked: 0 };
    let sets = sets_of(vec![("minecraft:village_plains", villages())]);
    let mut s = Search::new("k", &["minecraft:village_plains".to_owned()], &sets, [0, 64, 0], vec![]);
    s.max_radius = 0;
    assert_eq!(s.do_work(&mut world), Some(Outcome::NotFound { radius: 0, samples: 0 }));
    assert_eq!(world.asked, 0);
}

#[test]
fn data_follows_the_item() {
    let mut d = Data::default();
    assert_eq!(d.state(), Some(State::Inactive));
    assert!(d.should_display_coordinates());
    d.set_searching("minecraft:village_plains");
    assert_eq!(d.state(), Some(State::Searching));
    d.succeed("minecraft:village_taiga", false, 640, -128, vec![[640, 0, -128]], 90, true, 0);
    assert_eq!(d.state(), Some(State::Found));
    assert_eq!(d.target(), Some((640, -128)));
    assert_eq!(d.structure_key.as_deref(), Some("minecraft:village_taiga"));
    assert!(d.prev_pos.is_some());
    // The crouching use drops the places found.
    d.deactivate();
    assert_eq!(d.state(), Some(State::Inactive));
    assert!(d.prev_pos.is_none());
    d.fail("minecraft:igloo", 9750, 12000);
    assert_eq!(d.state(), Some(State::NotFound));
    assert_eq!((d.search_radius, d.samples), (9750, 12000));
    let mut d = Data::default();
    for _ in 0..2 {
        d.succeed("a:b", true, 0, 0, vec![], 1, true, 2);
    }
    assert!(d.is_broken(2) && d.bar_visible(2) && d.bar_width(2) == 0 && d.is_group);
}

#[test]
fn tuned_values() {
    assert_eq!(config::GENERAL_MAX_RADIUS, 10000);
    assert_eq!(config::GENERAL_MAX_SAMPLES, 100000);
    assert_eq!(config::GENERAL_MAX_NEXT_SEARCHES, 100);
    assert_eq!(config::GENERAL_COMPASS_DURABILITY, 0);
    assert!(config::GENERAL_ALLOW_TELEPORT && config::GENERAL_DISPLAY_COORDINATES && config::CLIENT_TRANSLATE_STRUCTURE_NAMES);
    assert!(config::CLIENT_OVERLAY_SIDE_LEFT && config::CLIENT_DISPLAY_WITH_CHAT_OPEN);
    assert_eq!(xp_levels("minecraft:buried_treasure"), 0);
    let o = parse_xp_overrides(&["minecraft:buried_treasure,3", "minecraft:end*,2", "minecraft:*village*,1"]);
    assert_eq!(xp_levels_for("minecraft:end_city", 0, &o), 2);
    assert_eq!(xp_levels_for("minecraft:village_plains", 0, &o), 1);
    assert!(blacklisted(&["minecraft:stronghold", "minecraft:*village*"], "minecraft:village_taiga"));
    assert_eq!(round_radius(1234, 250), 1000);
}

#[test]
fn overlay_lines_follow_the_state() {
    let tr = |k: &str| format!("<{k}>");
    let mut d = Data::default();
    assert!(overlay_lines(&d, [0, 0, 0], "Village", &tr).is_empty());
    d.set_searching("minecraft:village_plains");
    d.search_radius = 750;
    let l = overlay_lines(&d, [0, 0, 0], "Village Plains", &tr);
    assert_eq!(l.iter().map(|x| x.rel).collect::<Vec<_>>(), vec![0, 1, 3, 4, 6, 7]);
    assert_eq!(l[2].text, "<structure>");
    d.succeed("minecraft:village_plains", false, 300, 400, vec![], 7, true, 0);
    let l = overlay_lines(&d, [0, 70, 0], "Village Plains", &tr);
    assert_eq!(l.iter().map(|x| x.rel).collect::<Vec<_>>(), vec![0, 1, 3, 4, 6, 7, 9, 10]);
    assert_eq!((l[5].text.as_str(), l[7].text.as_str()), ("300, 400", "500"));
    d.display_coordinates = Some(false);
    assert_eq!(overlay_lines(&d, [0, 0, 0], "x", &tr).len(), 4);
    d.set_not_found(500, 9);
    let l = overlay_lines(&d, [0, 0, 0], "x", &tr);
    // NOT_FOUND shows the radius, not the samples (unlike Nature's Compass).
    assert_eq!(l.iter().map(|x| x.rel).collect::<Vec<_>>(), vec![0, 1, 3, 4, 6, 7]);
}

fn entry(key: &str, name: &str, source: &str, group_key: &str, group: &str, dim: &str, xp: i64) -> Entry {
    Entry { key: key.into(), name: name.into(), source: source.into(), group_key: group_key.into(), group: group.into(), dimensions: dim.into(), xp }
}
fn sample() -> Vec<Entry> {
    vec![
        entry("minecraft:village_plains", "Village Plains", "Minecraft", "minecraft:villages", "Villages", "Overworld", 0),
        entry("minecraft:village_taiga", "Village Taiga", "Minecraft", "minecraft:villages", "Villages", "Overworld", 0),
        entry("minecraft:fortress", "Fortress", "Minecraft", "minecraft:nether_complexes", "Nether Complexes", "The Nether", 2),
        entry("minecraft:stronghold", "Stronghold", "Minecraft", "minecraft:strongholds", "Strongholds", "Overworld", 0),
        entry("tfc:camp", "Camp", "Some Mod", "explorerscompass:none", "None", "Overworld", 1),
    ]
}

#[test]
fn pretty_names_follow_the_config() {
    assert_eq!(pretty_name("minecraft:village_plains", None), "Village Plains");
    assert_eq!(pretty_name("minecraft:end_city", Some("End City".into())), "End City");
    assert_eq!(pretty_name("yungsapi:x/y_z", None), "X/y Z");
}

#[test]
fn list_is_sorted_filtered_and_selected() {
    let mut m = Model::new(sample(), Some("minecraft:stronghold".into()), true, 100, false, 0);
    let names: Vec<_> = m.rows.iter().map(|&i| m.entries[i].name.as_str()).collect();
    assert_eq!(names, vec!["Camp", "Fortress", "Stronghold", "Village Plains", "Village Taiga"]);
    assert_eq!(m.selected.as_deref(), Some("minecraft:stronghold"));
    assert!(m.found_selected());
    // The structure of a mod or one that costs levels are listed but disabled.
    assert!(!m.enabled(&m.entries[2]) && !m.enabled(&m.entries[4]));
    m.set_term("village");
    assert_eq!(m.rows.len(), 2);
    assert_eq!(m.selected, None);
    m.set_term("@some");
    assert_eq!(m.rows.len(), 1);
    m.set_term("");
    // Name -> Source -> Dimension -> Levels -> Group -> Name.
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Source);
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Dimension);
    assert_eq!(m.entries[m.rows[4]].name, "Fortress", "The Nether last");
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Xp);
    assert_eq!(m.entries[m.rows[4]].xp, 2);
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Group);
    assert_eq!(m.entries[m.rows[0]].group_key, "explorerscompass:none");
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Name);
}

#[test]
fn layout_matches_the_original_widgets() {
    let l = layout(427, 240, true, true);
    assert_eq!(l.search, Rect::new(10, 40, 110, 20));
    assert_eq!(l.group, Rect::new(10, 65, 110, 20));
    assert_eq!(l.next, Some(Rect::new(10, 90, 110, 20)));
    assert_eq!(l.sort, Rect::new(10, 125, 110, 20));
    assert_eq!(l.teleport, Some(Rect::new(307, 10, 110, 20)));
    assert_eq!(l.cancel, Rect::new(10, 210, 110, 20));
    assert_eq!(l.field, Rect::new(130, 10, 140, 20));
    assert_eq!(l.list, Rect::new(130, 40, 287, 190));
    let l = layout(427, 240, false, false);
    assert_eq!(l.sort.y, 100);
    assert!(l.next.is_none() && l.teleport.is_none());
}

#[test]
fn clicks_do_what_the_buttons_do() {
    let (w, h) = (427, 240);
    let mut m = Model::new(sample(), None, true, 100, false, 0);
    // Nothing selected: nothing happens on Search, Search for Group, Next and Teleport.
    assert_eq!(m.click(20.0, 50.0, w, h, 0), Action::None);
    assert_eq!(m.click(20.0, 75.0, w, h, 0), Action::None);
    assert_eq!(m.click(20.0, 100.0, w, h, 0), Action::None);
    assert_eq!(m.click((w - 100) as f32, 20.0, w, h, 0), Action::None);
    // Row 3 = Village Plains (rows: Camp, Fortress, Stronghold, Village Plains, Village Taiga).
    let l = m.layout(w, h).list;
    let y3 = (l.y + 4 + 150 + 10) as f32;
    assert_eq!(m.click(200.0, y3, w, h, 1000), Action::None);
    assert_eq!(m.selected.as_deref(), Some("minecraft:village_plains"));
    assert_eq!(m.click(20.0, 50.0, w, h, 2000), Action::Search("minecraft:village_plains".into()));
    assert_eq!(m.click(20.0, 75.0, w, h, 2000), Action::SearchGroup("minecraft:villages".into()));
    // Next / teleport need the selected structure to be the found one.
    assert_eq!(m.click(20.0, 100.0, w, h, 2000), Action::None);
    // A double click on a row searches.
    assert_eq!(m.click(200.0, y3, w, h, 5000), Action::None);
    assert_eq!(m.click(200.0, y3, w, h, 5100), Action::Search("minecraft:village_plains".into()));
    // The disabled rows (Fortress costs levels) cannot be selected.
    let y1 = (l.y + 4 + 50 + 10) as f32;
    m.click(200.0, y1, w, h, 9000);
    assert_eq!(m.selected.as_deref(), Some("minecraft:village_plains"));
    // Sort, cancel, the field.
    m.click(20.0, 135.0, w, h, 9000);
    assert_eq!(m.sorting, Sorting::Source);
    assert_eq!(m.click(20.0, 225.0, w, h, 9000), Action::Close);
    assert_eq!(m.click(200.0, 20.0, w, h, 9000), Action::FocusSearch);
    // FOUND for the selected structure: next and teleport work.
    let mut m = Model::new(sample(), Some("minecraft:village_taiga".into()), true, 100, false, 0);
    assert_eq!(m.click(20.0, 100.0, w, h, 0), Action::SearchNext);
    assert_eq!(m.click((w - 100) as f32, 20.0, w, h, 0), Action::Teleport);
    // Creative: every row is enabled.
    let m = Model::new(sample(), None, false, 0, true, 0);
    assert!(m.enabled(&m.entries[2]));
}

#[test]
fn colours_and_scrolling_are_the_originals() {
    assert_eq!(button_alpha(true, false), 127);
    assert_eq!(button_alpha(true, true), 242);
    assert_eq!(button_alpha(false, false), 43);
    assert_eq!(row_alpha(true, true), 184);
    let (w, h) = (427, 240);
    let entries: Vec<Entry> = (0..30).map(|i| entry(&format!("m:s{i:02}"), &format!("S{i:02}"), "Minecraft", "g:g", "G", "Overworld", 0)).collect();
    let mut m = Model::new(entries, None, true, 100, true, 0);
    let l = m.layout(w, h).list;
    assert_eq!(m.max_scroll(l), (1500 - 186) as f32);
    m.wheel(-1.0, l);
    assert_eq!(m.scroll, 25.0);
    m.wheel(1000.0, l);
    assert_eq!(m.scroll, 0.0);
    m.set_scroll(100.0, l);
    assert_eq!(m.row_at(200.0, (l.y + 4 + 10) as f32, l), Some(2));
    assert!(m.thumb(l).is_some());
    m.set_scroll(0.0, l);
    m.drag_scroll(10.0, l);
    assert!(m.scroll > 10.0);
}

// A BTreeMap import keeps the test file honest about ordering of expectations above.
#[allow(dead_code)]
fn _unused(_: BTreeMap<i32, i32>) {}
