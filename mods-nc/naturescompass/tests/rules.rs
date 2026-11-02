use horizon_naturescompass::*;

/// A world of plains with round patches of another biome: (centre x, centre z, radius, name).
struct Patches(Vec<(i32, i32, i32, &'static str)>);
impl BiomeSource for Patches {
    fn noise_biomes(&mut self, quarts: &[[i32; 3]]) -> Vec<Option<String>> {
        quarts
            .iter()
            .map(|q| {
                let (x, z) = (q[0] * 4, q[2] * 4);
                for &(cx, cz, r, name) in &self.0 {
                    let (dx, dz) = (i64::from(x - cx), i64::from(z - cz));
                    if dx * dx + dz * dz <= i64::from(r) * i64::from(r) {
                        return Some(name.to_owned());
                    }
                }
                Some("minecraft:plains".to_owned())
            })
            .collect()
    }
}

fn run(w: &mut Worker, src: &mut Patches) -> Finish {
    assert!(w.start().is_none());
    for _ in 0..2_000_000 {
        if let Some(f) = w.do_work(src) {
            return f;
        }
    }
    panic!("the search never ended");
}

#[test]
fn glob_and_blacklist() {
    assert!(glob_matches("minecraft:*ocean*", "minecraft:deep_lukewarm_ocean"));
    assert!(glob_matches("minecraft:savanna", "minecraft:savanna"));
    assert!(!glob_matches("minecraft:savanna", "minecraft:savanna_plateau"));
    assert!(glob_matches("minecraft:???", "minecraft:abc"));
    assert!(!glob_matches("minecraft:???", "minecraft:abcd"));
    // `.` is literal.
    assert!(!glob_matches("a.c", "abc"));
    assert!(blacklisted(&["minecraft:desert", "minecraft:*caves"], "minecraft:lush_caves"));
    assert!(!blacklisted(&[], "minecraft:desert"));
    // the pack: no blacklist.
    assert!(config::GENERAL_BIOME_BLACKLIST.is_empty());
}

#[test]
fn xp_levels_follow_the_config() {
    let o = parse_xp_overrides(&["minecraft:deep_dark,3", "minecraft:end*,9", "bad", "a,b,c", "x,notanumber"]);
    assert_eq!(o.len(), 2);
    assert_eq!(xp_levels_for("minecraft:deep_dark", 0, &o), 3);
    // More than 3 is 3.
    assert_eq!(xp_levels_for("minecraft:end_highlands", 1, &o), 3);
    assert_eq!(xp_levels_for("minecraft:plains", 1, &o), 1);
    // the pack's defaults: free.
    assert_eq!(xp_levels("minecraft:deep_dark"), 0);
}

#[test]
fn tuned_values() {
    assert_eq!(config::GENERAL_MAX_SAMPLES, 50000);
    assert_eq!(config::GENERAL_RADIUS_MODIFIER, 2500);
    assert_eq!(config::GENERAL_SAMPLE_SPACE_MODIFIER, 16);
    assert_eq!(config::GENERAL_MAX_NEXT_SEARCHES, 25);
    assert_eq!(config::GENERAL_COMPASS_DURABILITY, 0);
    assert!(config::GENERAL_ALLOW_TELEPORT && config::GENERAL_DISPLAY_COORDINATES);
    assert!(config::CLIENT_DISPLAY_WITH_CHAT_OPEN && config::CLIENT_FIX_BIOME_NAMES && config::CLIENT_OVERLAY_SIDE_LEFT);
    assert_eq!(config::CLIENT_OVERLAY_LINE_OFFSET, 1);
}

#[test]
fn out_from_origin_is_mth() {
    assert_eq!(out_from_origin(64, -63, 320, 64), vec![64, 128, 0, 192, 256, 320]);
    // An origin outside the range is clamped first.
    assert_eq!(out_from_origin(1000, -63, 320, 64)[0], 320);
    assert_eq!(out_from_origin(0, 1, 10, 64), vec![1]);
}

#[test]
fn distance_rounds_like_java() {
    assert_eq!(distance([0, 64, 0], 3, 4), 5);
    assert_eq!(distance([10, 64, 10], 10, 10), 0);
    assert_eq!(distance([0, 0, 0], 1000, 1000), 1414);
    assert_eq!(Worker::round_radius(1499, 500), 1000);
    assert_eq!(Worker::round_radius(1500, 500), 1500);
}

#[test]
fn search_finds_the_nearest_patch_on_the_spiral() {
    // Sample space 16 * 4 = 64. A desert 700 blocks east of the start.
    let mut src = Patches(vec![(700, 0, 80, "minecraft:desert")]);
    let mut w = Worker::new("minecraft:desert", [0, 64, 0], vec![], -64, 320);
    match run(&mut w, &mut src) {
        Finish::Found { x, z, prev_pos, samples } => {
            // The spiral samples on a 64 grid from the start; the first sample inside the patch is found.
            assert_eq!((x % 64, z % 64), (0, 0));
            assert!((x - 700).abs() <= 80 && z.abs() <= 80, "{x} {z}");
            assert_eq!(prev_pos.len(), 1);
            assert!(samples > 100 && samples < 1000, "{samples}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn search_in_the_start_biome_finds_at_once() {
    let mut src = Patches(vec![]);
    let mut w = Worker::new("minecraft:plains", [123, 70, -45], vec![], -64, 320);
    assert_eq!(
        run(&mut w, &mut src),
        Finish::Found { x: 123, z: -45, prev_pos: vec![[123, 70, -45]], samples: 0 }
    );
}

#[test]
fn a_missing_biome_fails_within_the_limits() {
    let mut src = Patches(vec![]);
    // Few samples: ends on maxSamples. The radius is rounded to 500.
    let mut w = Worker::with("minecraft:mushroom_fields", [0, 64, 0], vec![], -64, 320, 16, 2500, 300);
    match run(&mut w, &mut src) {
        Finish::Failed { radius, samples } => {
            assert_eq!(radius % 500, 0);
            assert!(samples >= 300 && samples <= 302, "{samples}");
        }
        other => panic!("{other:?}"),
    }
    // maxRadius 0: fails before the first sample.
    let mut w = Worker::with("minecraft:mushroom_fields", [0, 64, 0], vec![], -64, 320, 16, 0, 50000);
    assert_eq!(w.start(), Some(Finish::Failed { radius: 0, samples: 0 }));
}

#[test]
fn search_for_next_skips_the_blob_it_found_and_finds_another() {
    // Two deserts: a big one east and another south. The first search finds the nearer, the second the other.
    let mut src = Patches(vec![(600, 0, 200, "minecraft:desert"), (0, 1500, 150, "minecraft:desert")]);
    let mut first = Worker::new("minecraft:desert", [0, 64, 0], vec![], -64, 320);
    let Finish::Found { x, z, prev_pos, .. } = run(&mut first, &mut src) else { panic!() };
    assert!(x > 300 && z.abs() < 250, "{x} {z}");
    let mut next = Worker::new("minecraft:desert", [0, 64, 0], prev_pos, -64, 320);
    let Finish::Found { x: x2, z: z2, prev_pos: p2, .. } = run(&mut next, &mut src) else { panic!() };
    assert!(x2.abs() < 200 && (z2 - 1500).abs() <= 150, "{x2} {z2}");
    assert_eq!(p2.len(), 2);
}

#[test]
fn radius_is_reported_every_500_blocks() {
    let mut src = Patches(vec![(5000, 0, 100, "minecraft:desert")]);
    let mut w = Worker::new("minecraft:desert", [0, 64, 0], vec![], -64, 320);
    let f = run(&mut w, &mut src);
    assert!(matches!(f, Finish::Found { .. }));
    assert!(!w.radius_updates.is_empty());
    assert!(w.radius_updates.windows(2).all(|p| p[0] < p[1]));
    assert!(w.radius_updates.iter().all(|r| r % 500 == 0 && *r >= 500));
}

#[test]
fn data_follows_the_item() {
    let mut d = Data::default();
    assert_eq!(d.state(), Some(State::Inactive));
    assert!(d.should_display_coordinates());
    d.set_searching("minecraft:desert");
    assert_eq!(d.state(), Some(State::Searching));
    assert_eq!(d.target(), None);
    d.succeed("minecraft:desert", 640, -128, vec![[640, 64, -128]], 90, true, 0);
    assert_eq!(d.state(), Some(State::Found));
    assert_eq!(d.target(), Some((640, -128)));
    assert_eq!(d.compass_damage, 0);
    assert!(!d.is_broken(0));
    d.fail("minecraft:mushroom_fields", 2500, 50000);
    assert_eq!(d.state(), Some(State::NotFound));
    assert_eq!((d.search_radius, d.samples), (2500, 50000));
    // Durability: every success wears it.
    let mut d = Data::default();
    for _ in 0..3 {
        d.succeed("a:b", 0, 0, vec![], 1, true, 3);
    }
    assert!(d.is_broken(3));
    assert!(d.bar_visible(3));
    assert_eq!(d.bar_width(3), 0);
    assert_eq!(Data::default().bar_width(0), 13);
    assert_eq!(State::from_id(9), None);
}

#[test]
fn overlay_lines_follow_the_state() {
    let tr = |k: &str| format!("<{k}>");
    let mut d = Data::default();
    assert!(overlay_lines(&d, [0, 0, 0], "Desert", &tr).is_empty());
    d.set_searching("minecraft:desert");
    d.search_radius = 1500;
    let l = overlay_lines(&d, [0, 0, 0], "Desert", &tr);
    assert_eq!(l.iter().map(|x| x.rel).collect::<Vec<_>>(), vec![0, 1, 3, 4, 6, 7]);
    assert_eq!(l[5].text, "1500");
    d.succeed("minecraft:desert", 300, 400, vec![], 7, true, 0);
    let l = overlay_lines(&d, [0, 70, 0], "Desert", &tr);
    assert_eq!(l.iter().map(|x| x.rel).collect::<Vec<_>>(), vec![0, 1, 3, 4, 6, 7, 9, 10]);
    assert_eq!(l[5].text, "300, 400");
    assert_eq!(l[7].text, "500");
    assert_eq!(l[0].color, WHITE);
    assert_eq!(l[1].color, GREY);
    // No coordinates when the compass was told not to.
    d.display_coordinates = Some(false);
    assert_eq!(overlay_lines(&d, [0, 0, 0], "Desert", &tr).len(), 4);
    d.fail("minecraft:desert", 2500, 77);
    let l = overlay_lines(&d, [0, 0, 0], "Desert", &tr);
    assert_eq!(l.iter().map(|x| x.rel).collect::<Vec<_>>(), vec![0, 1, 3, 4, 6, 7, 9, 10]);
    assert_eq!(l[7].text, "77");
}

#[test]
fn hud_lines_sit_on_the_left_with_tuned_offsets() {
    // (rel + 1) * 9 + 5 + 2 ; x = 5 + 2
    assert_eq!(hud_position(0, 427.0, 50.0), (7.0, 16.0, true));
    assert_eq!(hud_position(4, 427.0, 50.0), (7.0, 52.0, true));
}

struct Col {
    solid: Vec<i32>,
    min: i32,
    max: i32,
}
impl Column for Col {
    fn free(&self, y: i32) -> bool {
        !self.solid.contains(&y)
    }
    fn outside(&self, y: i32) -> bool {
        y < self.min || y >= self.max
    }
}

#[test]
fn teleport_height_finds_a_standing_spot_near_sea_level() {
    // Solid up to y = 70: stand at 71.
    let c = Col { solid: (-64..=70).collect(), min: -64, max: 320 };
    assert_eq!(teleport_height(&c, 63), 71);
    // Solid up to 60 and a 2-high gap above a ledge at 80..: the nearer spot (61).
    let c = Col { solid: (-64..=60).chain(80..=90).collect(), min: -64, max: 320 };
    assert_eq!(teleport_height(&c, 63), 61);
    // Nothing solid at all: 256.
    let c = Col { solid: vec![], min: -64, max: 320 };
    assert_eq!(teleport_height(&c, 63), 256);
    // Water is free: standing on the sea floor (water over sand at 40).
    let c = Col { solid: (-64..=40).collect(), min: -64, max: 320 };
    assert_eq!(teleport_height(&c, 63), 41);
}

fn entry(key: &str, name: &str, source: &str, tags: &str, dim: &str, xp: i64, rain: f32, temp: f32) -> Entry {
    Entry {
        key: key.into(),
        name: name.into(),
        display: fix_biome_name(name),
        source: source.into(),
        tags: tags.into(),
        dimensions: dim.into(),
        xp,
        rainfall: rain,
        temperature: temp,
    }
}
fn sample() -> Vec<Entry> {
    vec![
        entry("minecraft:plains", "Plains", "Minecraft", "None", "Overworld", 0, 0.4, 0.8),
        entry("minecraft:desert", "Desert", "Minecraft", "None", "Overworld", 0, 0.0, 2.0),
        entry("minecraft:forest", "Forest", "Minecraft", "Forest", "Overworld", 1, 0.8, 0.7),
        entry("minecraft:nether_wastes", "Nether Wastes", "Minecraft", "Nether", "The Nether", 0, 0.0, 2.0),
        entry("tfc:dark_forest", "DarkForest", "Some Mod", "Forest", "Overworld", 2, 0.9, 0.7),
    ]
}

#[test]
fn names_get_their_spaces_and_capitals() {
    assert_eq!(fix_biome_name("ForestHills"), "Forest Hills");
    assert_eq!(fix_biome_name("Plains"), "Plains");
    assert_eq!(fix_biome_name("Flower forest"), "Flower forest");
    assert_eq!(capitalize("wooded hills"), "Wooded Hills");
    assert_eq!(java_float(0.8), "0.8");
    assert_eq!(java_float(2.0), "2.0");
    assert_eq!(java_float(-0.5), "-0.5");
}

#[test]
fn list_is_sorted_filtered_and_selected() {
    let mut m = Model::new(sample(), Some("minecraft:desert".into()), true, 25, false, 0);
    // Sorted by name; the found biome is pre-selected; entries that cost levels are listed but disabled.
    let names: Vec<_> = m.rows.iter().map(|&i| m.entries[i].name.as_str()).collect();
    assert_eq!(names, vec!["DarkForest", "Desert", "Forest", "Nether Wastes", "Plains"]);
    assert_eq!(m.selected.as_deref(), Some("minecraft:desert"));
    assert!(m.found_selected());
    assert!(!m.enabled(&m.entries[2]));
    // Search by name (shown with its spaces), by tag with $, by source with @.
    m.set_term("dark f");
    assert_eq!(m.rows.len(), 1);
    assert_eq!(m.selected, None, "the selected desert is no longer listed");
    m.set_term("$nether");
    assert_eq!(m.rows.len(), 1);
    m.set_term("@some");
    assert_eq!(m.entries[m.rows[0]].key, "tfc:dark_forest");
    m.set_term("");
    assert_eq!(m.rows.len(), 5);
    // The sorting cycles Name -> Source -> Tags -> Dimension -> Levels -> Rainfall -> Temperature -> Name.
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Source);
    let first = &m.entries[m.rows[4]];
    assert_eq!(first.source, "Some Mod");
    for _ in 0..3 {
        m.cycle_sorting();
    }
    assert_eq!(m.sorting, Sorting::Xp);
    assert_eq!(m.entries[m.rows[4]].xp, 2);
    m.cycle_sorting();
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Temperature);
    // Desert and Nether Wastes tie at 2.0: the name order stays (stable sort).
    assert_eq!(m.entries[m.rows[4]].name, "Nether Wastes");
    m.cycle_sorting();
    assert_eq!(m.sorting, Sorting::Name);
    assert_eq!(Sorting::Rainfall.value(&m.entries[0]), "0.4");
}

#[test]
fn clicks_do_what_the_buttons_do() {
    let (w, h) = (427, 240);
    let mut m = Model::new(sample(), None, true, 25, false, 0);
    let l = m.layout(w, h);
    assert_eq!(l.sort.y, 100);
    // Nothing selected: Start Search, Search for Next and Teleport do nothing.
    assert_eq!(m.click(20.0, 50.0, w, h, 0), Action::None);
    assert_eq!(m.click(20.0, 75.0, w, h, 0), Action::None);
    assert_eq!(m.click((w - 100) as f32, 20.0, w, h, 0), Action::None);
    // Click the second row (Desert): selected; the start button then searches.
    let row1_y = (l.list.y + 4 + 50 + 10) as f32;
    assert_eq!(m.click(200.0, row1_y, w, h, 1000), Action::None);
    assert_eq!(m.selected.as_deref(), Some("minecraft:desert"));
    assert_eq!(m.click(20.0, 50.0, w, h, 2000), Action::Search("minecraft:desert".into()));
    // Not FOUND: still no next / teleport.
    assert_eq!(m.click(20.0, 75.0, w, h, 2000), Action::None);
    // A double click (within 250 ms) on a row searches.
    assert_eq!(m.click(200.0, row1_y, w, h, 5000), Action::None);
    assert_eq!(m.click(200.0, row1_y, w, h, 5100), Action::Search("minecraft:desert".into()));
    // Slow second click does not.
    assert_eq!(m.click(200.0, row1_y, w, h, 6000), Action::None);
    // A disabled row (Forest costs a level) is not selectable.
    let row2_y = (l.list.y + 4 + 100 + 10) as f32;
    assert_eq!(m.click(200.0, row2_y, w, h, 7000), Action::None);
    assert_eq!(m.selected.as_deref(), Some("minecraft:desert"));
    // Sort button cycles; cancel closes; the search box takes the focus.
    assert_eq!(m.click(20.0, 110.0, w, h, 8000), Action::None);
    assert_eq!(m.sorting, Sorting::Source);
    assert_eq!(m.click(20.0, (h - 20) as f32, w, h, 8000), Action::Close);
    assert_eq!(m.click(200.0, 20.0, w, h, 8000), Action::FocusSearch);
    // With a FOUND compass for the selected biome: next and teleport work.
    let mut m = Model::new(sample(), Some("minecraft:plains".into()), true, 25, false, 0);
    assert_eq!(m.click(20.0, 75.0, w, h, 0), Action::SearchNext);
    assert_eq!(m.click((w - 100) as f32, 20.0, w, h, 0), Action::Teleport);
    // Without next searches and teleport the buttons are not there and the sort button moves up.
    let m = Model::new(sample(), None, false, 0, true, 0);
    let l = m.layout(w, h);
    assert!(l.next.is_none() && l.teleport.is_none());
    assert_eq!(l.sort.y, 65);
    assert!(m.enabled(&m.entries[2]), "infinite XP (creative) enables every row");
}

#[test]
fn list_scrolls_by_half_a_row_per_notch() {
    let (w, h) = (427, 240);
    let mut entries = Vec::new();
    for i in 0..30 {
        entries.push(entry(&format!("m:b{i:02}"), &format!("B{i:02}"), "Minecraft", "None", "Overworld", 0, 0.5, 0.5));
    }
    let mut m = Model::new(entries, None, true, 25, true, 0);
    let l = m.layout(w, h).list;
    assert_eq!(l, Rect::new(130, 40, 287, 190));
    assert_eq!(m.max_position(), 1500);
    assert_eq!(m.max_scroll(l), (1500 - 186) as f32);
    m.wheel(-1.0, l);
    assert_eq!(m.scroll, 25.0);
    m.wheel(1.0, l);
    m.wheel(1.0, l);
    assert_eq!(m.scroll, 0.0, "never above the top");
    m.wheel(-1000.0, l);
    assert_eq!(m.scroll, m.max_scroll(l), "never below the end");
    // After scrolling, a click lands on the row under the pointer.
    m.set_scroll(100.0, l);
    let row = m.row_at(200.0, (l.y + 4 + 10) as f32, l).unwrap();
    assert_eq!(row, 2);
    assert_eq!(m.row_top(2, l), l.y + 4 - 100 + 100);
    // The thumb.
    let (top, th) = m.thumb(l).unwrap();
    assert!(top >= l.y && th >= 32 && th <= l.h - 8);
    // Dragging the thumb by the pointer.
    m.set_scroll(0.0, l);
    m.drag_scroll(10.0, l);
    assert!(m.scroll > 10.0);
}

#[test]
fn button_colours_are_the_originals() {
    // TransparentButton: normal 127, hovered 242, inactive 43 (the shift loses the high bits).
    assert_eq!(button_alpha(true, false), 127);
    assert_eq!(button_alpha(true, true), 242);
    assert_eq!(button_alpha(false, false), 43);
    assert_eq!(button_alpha(false, true), 43);
    // The list rows: normal 127, selected 184, disabled 43.
    assert_eq!(row_alpha(true, false), 127);
    assert_eq!(row_alpha(true, true), 184);
    assert_eq!(row_alpha(false, false), 43);
}

#[test]
fn layout_matches_the_original_widgets() {
    let l = layout(427, 240, true, true);
    assert_eq!(l.start, Rect::new(10, 40, 110, 20));
    assert_eq!(l.next, Some(Rect::new(10, 65, 110, 20)));
    assert_eq!(l.sort, Rect::new(10, 100, 110, 20));
    assert_eq!(l.teleport, Some(Rect::new(307, 10, 110, 20)));
    assert_eq!(l.cancel, Rect::new(10, 210, 110, 20));
    assert_eq!(l.search, Rect::new(130, 10, 140, 20));
}
