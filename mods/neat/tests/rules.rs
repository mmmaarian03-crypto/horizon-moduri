use horizon_neat::{
    Config, Env, MobKind, Plate, PlateQuad, Subject, bar_color, format_decimal, hsv_to_rgb, is_boss, is_friendly,
    mob_kind, plate, should_show,
};

fn env() -> Env {
    Env { render_names: true, drawing: true, debug: false }
}
fn zombie() -> Subject {
    Subject {
        id: "minecraft:zombie".into(),
        player: false,
        friendly: false,
        monster: true,
        health: 20.0,
        max_health: 20.0,
        armor: 0,
        name: "Zombie".into(),
        custom_name: false,
        distance: 10.0,
        line_of_sight: true,
        looked_at: false,
        carries_camera: false,
        visible: true,
    }
}
fn cow() -> Subject {
    Subject {
        id: "minecraft:cow".into(),
        friendly: true,
        monster: false,
        health: 10.0,
        max_health: 10.0,
        name: "Cow".into(),
        ..zombie()
    }
}
/// A font of 6 pixels a character.
fn width(text: &str) -> f32 {
    text.chars().count() as f32 * 6.0
}

// ---- the config ----

#[test]
fn tuned_config_is_the_default() {
    // config\neat-client.toml
    let toml = r##"
max_distance = 24
max_distance_without_line_of_sight = 8
render_without_gui = false
height_above_mob = 0.6
draw_background = true
background_padding = 2
background_height = 6
health_bar_height = 4
plate_size = 25
plate_size_boss = 50
show_attributes = true
show_armor = true
group_armor = true
color_health_bar_by_type = false
text_color = "FFFFFF"
hp_text_height = 14
show_max_hp = true
show_current_hp = true
show_hp_percentage = true
show_on_passive = true
show_on_hostile = true
display_on_players = true
display_on_bosses = true
only_health_bar_for_target = false
show_entity_full_health = true
show_debug_with_f3 = true
show_entity_name = true
disable_name_tag = false
disable_name_tag_if_healthbar = true
icon_offset_x = 0.0
icon_offset_y = 0.0
decimal_format = "#.##"
blacklist = ["minecraft:shulker", "minecraft:armor_stand", "minecraft:cod", "minecraft:salmon", "minecraft:pufferfish", "minecraft:tropical_fish", "minecraft:tadpole"]
"##;
    let get = |key: &str| {
        toml.lines()
            .find_map(|l| l.strip_prefix(key).and_then(|r| r.trim_start().strip_prefix('=')))
            .map(|v| v.trim().to_owned())
    };
    assert_eq!(Config::read(get), Config::default());
}

#[test]
fn another_value_is_read_and_a_bad_one_is_the_default() {
    let c = Config::read(|k| match k {
        "max_distance" => Some("40".into()),
        "plate_size" => Some("abc".into()),
        "show_armor" => Some("false".into()),
        "blacklist" => Some(r#"["minecraft:bat"]"#.into()),
        "text_color" => Some("\"FF0000\"".into()),
        _ => None,
    });
    assert_eq!(c.max_distance, 40);
    assert_eq!(c.plate_size, 25);
    assert!(!c.show_armor);
    assert_eq!(c.blacklist, ["minecraft:bat"]);
    assert_eq!(c.text_color, "FF0000");
}

// ---- who gets a plate ----

#[test]
fn a_mob_in_range_gets_a_plate() {
    assert!(should_show(&Config::default(), env(), &zombie()));
    assert!(should_show(&Config::default(), env(), &cow()));
}

#[test]
fn the_distance_rules() {
    let cfg = Config::default();
    let mut z = zombie();
    z.distance = 24.0;
    assert!(should_show(&cfg, env(), &z));
    z.distance = 24.1;
    assert!(!should_show(&cfg, env(), &z));
    // Beyond 8 blocks it needs a line of sight; within them it does not.
    z.distance = 8.5;
    z.line_of_sight = false;
    assert!(!should_show(&cfg, env(), &z));
    z.distance = 8.0;
    assert!(should_show(&cfg, env(), &z));
}

#[test]
fn the_blacklist_uses_entity_ids() {
    let cfg = Config::default();
    let mut s = zombie();
    for id in ["minecraft:shulker", "minecraft:armor_stand", "minecraft:cod", "minecraft:salmon", "minecraft:pufferfish", "minecraft:tropical_fish", "minecraft:tadpole"] {
        s.id = id.into();
        assert!(!should_show(&cfg, env(), &s), "{id}");
    }
    s.id = "minecraft:bat".into();
    assert!(should_show(&cfg, env(), &s));
}

#[test]
fn the_f1_toggle_and_the_key() {
    let cfg = Config::default();
    let s = zombie();
    assert!(!should_show(&cfg, Env { render_names: false, ..env() }, &s));
    let in_f1 = Config { render_without_gui: true, ..Config::default() };
    assert!(should_show(&in_f1, Env { render_names: false, ..env() }, &s));
    assert!(!should_show(&cfg, Env { drawing: false, ..env() }, &s));
}

#[test]
fn the_kind_filters() {
    let mut cfg = Config::default();
    cfg.show_on_passive = false;
    assert!(!should_show(&cfg, env(), &cow()));
    assert!(should_show(&cfg, env(), &zombie()));
    let mut cfg = Config::default();
    cfg.show_on_hostile = false;
    assert!(should_show(&cfg, env(), &cow()));
    assert!(!should_show(&cfg, env(), &zombie()));
    // A boss is not "hostile" for that option, and a player (category misc) is.
    let mut wither = zombie();
    wither.id = "minecraft:wither".into();
    assert!(should_show(&cfg, env(), &wither));
    let mut player = zombie();
    player.id = "minecraft:player".into();
    player.player = true;
    assert!(!should_show(&cfg, env(), &player));
    let mut cfg = Config::default();
    cfg.display_on_players = false;
    assert!(!should_show(&cfg, env(), &player));
    cfg.display_on_bosses = false;
    assert!(!should_show(&cfg, env(), &wither));
}

#[test]
fn full_health_target_riding_and_visibility() {
    let mut cfg = Config::default();
    cfg.show_entity_full_health = false;
    assert!(!should_show(&cfg, env(), &zombie()));
    let mut hurt = zombie();
    hurt.health = 19.0;
    assert!(should_show(&cfg, env(), &hurt));
    let mut cfg = Config::default();
    cfg.only_health_bar_for_target = true;
    assert!(!should_show(&cfg, env(), &zombie()));
    let mut looked = zombie();
    looked.looked_at = true;
    assert!(should_show(&cfg, env(), &looked));
    let mut mount = zombie();
    mount.carries_camera = true;
    assert!(!should_show(&Config::default(), env(), &mount));
    let mut unseen = zombie();
    unseen.visible = false;
    assert!(!should_show(&Config::default(), env(), &unseen));
}

#[test]
fn mob_kinds_bosses_and_categories() {
    assert_eq!(mob_kind("minecraft:spider"), MobKind::Arthropod);
    assert_eq!(mob_kind("minecraft:bee"), MobKind::Arthropod);
    assert_eq!(mob_kind("minecraft:silverfish"), MobKind::Arthropod);
    assert_eq!(mob_kind("minecraft:zombie"), MobKind::Undead);
    assert_eq!(mob_kind("minecraft:zombified_piglin"), MobKind::Undead);
    assert_eq!(mob_kind("minecraft:phantom"), MobKind::Undead);
    assert_eq!(mob_kind("minecraft:skeleton_horse"), MobKind::Undead);
    assert_eq!(mob_kind("minecraft:pillager"), MobKind::Illager);
    assert_eq!(mob_kind("minecraft:evoker"), MobKind::Illager);
    // Not illagers: the ravager, the witch and the vex.
    assert_eq!(mob_kind("minecraft:ravager"), MobKind::Undefined);
    assert_eq!(mob_kind("minecraft:witch"), MobKind::Undefined);
    assert_eq!(mob_kind("minecraft:vex"), MobKind::Undefined);
    assert_eq!(mob_kind("minecraft:cow"), MobKind::Undefined);
    assert!(is_boss("minecraft:ender_dragon") && is_boss("minecraft:wither"));
    assert!(!is_boss("minecraft:warden"));
    assert!(is_friendly("creature") && is_friendly("ambient") && is_friendly("axolotls") && is_friendly("water_creature"));
    assert!(!is_friendly("monster") && !is_friendly("misc"));
}

// ---- the colours ----

#[test]
fn hsv_is_minecrafts() {
    assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), 0xFF0000);
    assert_eq!(hsv_to_rgb(1.0 / 3.0, 1.0, 1.0) >> 8 & 0xFF, 0xFF);
    // Mth.hsvToRgb(0.58 / 6.0, 1, 1): sextant 0 with f = 0.58 -> (255, 147, 0).
    assert_eq!(hsv_to_rgb(0.58 / 6.0, 1.0, 1.0), 0xFF_93_00);
    // Sextant 1 with f = 0.58 -> (107, 255, 0).
    assert_eq!(hsv_to_rgb(1.58 / 6.0, 1.0, 1.0), 0x6B_FF_00);
}

#[test]
fn the_bar_goes_from_green_to_red_with_the_health() {
    let cfg = Config::default();
    let mut s = zombie();
    // Full health: hue 1/3 - 0.07.
    let full = bar_color(&cfg, &s);
    assert_eq!(full >> 8 & 0xFF, 255);
    assert!(full >> 16 & 0xFF > 90 && full >> 16 & 0xFF < 120, "{full:06X}");
    s.health = 10.0;
    assert_eq!(bar_color(&cfg, &s), 0xFF_93_00 | 0, "{:06X}", bar_color(&cfg, &s));
    s.health = 0.0;
    assert_eq!(bar_color(&cfg, &s), 0xFF0000);
    // More health than the maximum counts as full.
    s.health = 40.0;
    assert_eq!(bar_color(&cfg, &s), full);
    let by_type = Config { color_health_bar_by_type: true, ..Config::default() };
    assert_eq!(bar_color(&by_type, &zombie()), 0xFF0000);
    assert_eq!(bar_color(&by_type, &cow()), 0x00FF00);
    let mut w = zombie();
    w.id = "minecraft:wither".into();
    assert_eq!(bar_color(&by_type, &w), 0x800080);
}

// ---- DecimalFormat ----

#[test]
fn decimal_format_hash_hash() {
    let f = |v: f64| format_decimal(v, "#.##");
    assert_eq!(f(20.0), "20");
    assert_eq!(f(14.5), "14.5");
    assert_eq!(f(3.14159), "3.14");
    assert_eq!(f(0.0), "0");
    // No leading zero before a fraction, as DecimalFormat("#.##") prints it.
    assert_eq!(f(0.5), ".5");
    assert_eq!(f(0.05), ".05");
    // HALF_EVEN on the double's exact value: 0.125 is a tie (to the even 2), 2.675 is a hair under.
    assert_eq!(f(0.125), ".12");
    assert_eq!(f(0.375), ".38");
    assert_eq!(f(2.675), "2.67");
    assert_eq!(f(0.135), ".14");
    // A float health widened to double.
    assert_eq!(f(f64::from(20.1_f32)), "20.1");
    assert_eq!(f(f64::from(0.3_f32)), ".3");
    // Carry into the integer part.
    assert_eq!(f(9.999), "10");
    assert_eq!(f(0.999), "1");
    assert_eq!(f(-1.5), "-1.5");
}

#[test]
fn decimal_format_with_zeros() {
    assert_eq!(format_decimal(3.5, "0.00"), "3.50");
    assert_eq!(format_decimal(0.5, "0.00"), "0.50");
    assert_eq!(format_decimal(7.0, "0.0"), "7.0");
    assert_eq!(format_decimal(7.26, "0.#"), "7.3");
    assert_eq!(format_decimal(1234.5, "#"), "1234");
    assert_eq!(format_decimal(2.5, "#"), "2");
    assert_eq!(format_decimal(3.5, "#"), "4");
    assert_eq!(format_decimal(5.0, "00"), "05");
}

// ---- the plate ----

fn quads_of(p: &Plate, depth: u8) -> Vec<PlateQuad> {
    p.quads.iter().copied().filter(|q| q.depth == depth).collect()
}

#[test]
fn a_full_zombie_plate() {
    let cfg = Config::default();
    let p = plate(&cfg, env(), &zombie(), &width);
    // "Zombie" is 36 wide, 18 at the name scale: 9 + 10 < 25, so the plate keeps its 25.
    assert_eq!(p.half_size, 25.0);
    // The background: black at 64, 2 around the bar, 6 above it.
    let bg = quads_of(&p, 2);
    assert_eq!(bg, vec![PlateQuad { x0: -27.0, y0: -6.0, x1: 27.0, y1: 6.0, rgba: [0, 0, 0, 64], depth: 2 }]);
    // The bar covers it all (full health), at 127, the colour of a full health.
    let bar = quads_of(&p, 1);
    assert_eq!(bar.len(), 1);
    assert_eq!((bar[0].x0, bar[0].x1, bar[0].y0, bar[0].y1), (-25.0, 25.0, 0.0, 4.0));
    assert_eq!(bar[0].rgba[3], 127);
    assert_eq!(bar[0].rgba[1], 255);
    // The texts: name, current, max, percentage.
    let texts: Vec<_> = p.texts.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(texts, ["Zombie", "20", "20", "100%"]);
    // The name at the plate's left, 4.5 above the bar's top, half size.
    assert_eq!((p.texts[0].x, p.texts[0].y, p.texts[0].scale), (-25.0, -4.5, 0.5));
    // The values at scale 0.375: 14 font pixels down from the same origin = 0.75 into the bar.
    assert_eq!(p.texts[1].scale, 0.375);
    assert!((p.texts[1].y - 0.75).abs() < 1e-6);
    // Current at x 2, max right-aligned at (int)(25 / 0.375 * 2) - bold width ("20": 12 + 2) - 2 = 117,
    // the percentage centred at (int)(25 / 0.375) - 24 / 2 = 54.
    assert!((p.texts[1].x - (-25.0 + 2.0 * 0.375)).abs() < 1e-5);
    assert!((p.texts[2].x - (-25.0 + 117.0 * 0.375)).abs() < 1e-5);
    assert!(p.texts[2].bold && !p.texts[1].bold);
    assert!((p.texts[3].x - (-25.0 + 54.0 * 0.375)).abs() < 1e-5);
    assert!(p.texts.iter().all(|t| t.rgb == 0xFFFFFF));
    // An undead mob shows the rotten flesh 2.85 units in from the plate's right end (x is the camera's left, negated), 3 units above.
    assert_eq!(p.icons.len(), 1);
    assert_eq!(p.icons[0].item, "minecraft:rotten_flesh");
    assert!((p.icons[0].offset[0] + (25.0 - 2.85) * 0.0267).abs() < 1e-5);
    assert!((p.icons[0].offset[1] - 3.0 * 0.0267).abs() < 1e-6);
    assert_eq!(p.icons[0].offset[2], 0.0);
}

#[test]
fn a_hurt_mob_has_a_short_bar_and_a_black_rest() {
    let cfg = Config::default();
    let mut z = zombie();
    z.health = 5.0;
    let p = plate(&cfg, env(), &z, &width);
    let bar = quads_of(&p, 1);
    assert_eq!(bar.len(), 2);
    // A quarter of the 50 wide bar: 12.5 -> 2 * 6.25 * ... = x from -25 to -12.5.
    assert!((bar[0].x1 - -12.5).abs() < 1e-5, "{bar:?}");
    assert_eq!(bar[1].rgba, [0, 0, 0, 127]);
    assert!((bar[1].x0 - -12.5).abs() < 1e-5);
    assert_eq!(bar[1].x1, 25.0);
    assert_eq!(p.texts[3].text, "25%");
}

#[test]
fn a_long_name_widens_the_plate() {
    let cfg = Config::default();
    let mut z = zombie();
    z.name = "A very long custom name here".into();
    z.custom_name = true;
    let p = plate(&cfg, env(), &z, &width);
    // 28 chars * 6 = 168 * 0.5 = 84, half 42, + 10 = 52.
    assert_eq!(p.half_size, 52.0);
    assert!(p.texts[0].italic);
    let bg = quads_of(&p, 2);
    assert_eq!(bg[0].x0, -54.0);
}

#[test]
fn overhealed_mobs_do_not_overflow_the_bar() {
    let cfg = Config::default();
    let mut z = zombie();
    z.health = 30.0;
    let p = plate(&cfg, env(), &z, &width);
    let bar = quads_of(&p, 1);
    assert_eq!(bar.len(), 1);
    assert_eq!(bar[0].x1, 25.0);
    // The text still reads the real numbers: 30 of 20.
    assert_eq!((p.texts[1].text.as_str(), p.texts[2].text.as_str(), p.texts[3].text.as_str()), ("30", "20", "150%"));
}

#[test]
fn texts_and_background_follow_the_options() {
    let mut cfg = Config::default();
    cfg.show_entity_name = false;
    cfg.show_max_hp = false;
    cfg.show_hp_percentage = false;
    let p = plate(&cfg, env(), &zombie(), &width);
    assert_eq!(p.texts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["20"]);
    // Without the name the background is 4 shorter.
    assert_eq!(quads_of(&p, 2)[0].y0, -2.0);
    cfg.draw_background = false;
    assert!(quads_of(&plate(&cfg, env(), &zombie(), &width), 2).is_empty());
    cfg.show_current_hp = false;
    assert!(plate(&cfg, env(), &zombie(), &width).texts.is_empty());
}

#[test]
fn the_f3_screen_adds_the_id() {
    let cfg = Config::default();
    let p = plate(&cfg, Env { debug: true, ..env() }, &zombie(), &width);
    let id = p.texts.last().unwrap();
    assert_eq!(id.text, "ID: \"minecraft:zombie\"");
    // 14 + 16 font pixels down.
    assert!((id.y - (-4.5 + 30.0 * 0.375)).abs() < 1e-5);
    let off = Config { show_debug_with_f3: false, ..Config::default() };
    assert_eq!(plate(&off, Env { debug: true, ..env() }, &zombie(), &width).texts.len(), 4);
}

#[test]
fn armor_is_grouped_into_diamond_icons() {
    let cfg = Config::default();
    let mut z = cow();
    z.armor = 7;
    let p = plate(&cfg, env(), &z, &width);
    // A cow has no attribute icon; 7 armor points are 2 iron chestplates and a diamond one, each 1 unit
    // further along (toward the centre) and 0.1 units nearer the camera.
    let items: Vec<_> = p.icons.iter().map(|i| i.item).collect();
    assert_eq!(items, ["minecraft:iron_chestplate", "minecraft:iron_chestplate", "minecraft:diamond_chestplate"]);
    // The attribute slot still took 5 units and 0.1 of depth: the first armour icon is at 2.85 + 5.
    assert!((p.icons[0].offset[0] + (25.0 - 7.85) * 0.0267).abs() < 1e-5);
    assert!((p.icons[0].offset[2] - -0.1 * 0.0267).abs() < 1e-6);
    assert!((p.icons[1].offset[0] + (25.0 - 8.85) * 0.0267).abs() < 1e-5);
    assert!((p.icons[2].offset[0] + (25.0 - 9.85) * 0.0267).abs() < 1e-5);
    assert!((p.icons[2].offset[2] - -0.3 * 0.0267).abs() < 1e-6);
    // Ungrouped: seven iron ones.
    let flat = Config { group_armor: false, ..Config::default() };
    assert_eq!(plate(&flat, env(), &z, &width).icons.len(), 7);
    // 10 armor: two diamond ones and no iron.
    z.armor = 10;
    let p = plate(&cfg, env(), &z, &width);
    assert_eq!(p.icons.iter().filter(|i| i.item == "minecraft:diamond_chestplate").count(), 2);
    assert_eq!(p.icons.len(), 2);
    // Options off.
    let none = Config { show_armor: false, ..Config::default() };
    assert!(plate(&none, env(), &z, &width).icons.is_empty());
}

#[test]
fn the_attribute_icons() {
    let cfg = Config::default();
    let item = |id: &str| {
        let mut s = zombie();
        s.id = id.into();
        plate(&cfg, env(), &s, &width).icons.first().map(|i| i.item)
    };
    assert_eq!(item("minecraft:spider"), Some("minecraft:spider_eye"));
    assert_eq!(item("minecraft:zombie"), Some("minecraft:rotten_flesh"));
    assert_eq!(item("minecraft:vindicator"), Some("minecraft:iron_axe"));
    assert_eq!(item("minecraft:wither"), Some("minecraft:nether_star"));
    assert_eq!(item("minecraft:creeper"), None);
    let off = Config { show_attributes: false, ..Config::default() };
    assert!(plate(&off, env(), &zombie(), &width).icons.is_empty());
    // The icon offset options move every icon.
    let moved = Config { icon_offset_x: 0.5, icon_offset_y: 0.25, ..Config::default() };
    let p = plate(&moved, env(), &zombie(), &width);
    assert!((p.icons[0].offset[0] + (25.0 - 2.85) * 0.0267 - 0.5).abs() < 1e-5 || (p.icons[0].offset[0] - (-(25.0 - 2.85) * 0.0267 - 0.5)).abs() < 1e-5);
    assert!((p.icons[0].offset[1] - (3.0 * 0.0267 + 0.25)).abs() < 1e-6);
}

#[test]
fn a_nan_health_draws_no_bar() {
    let cfg = Config::default();
    let mut z = zombie();
    z.health = 0.0;
    z.max_health = 0.0;
    let p = plate(&cfg, env(), &z, &width);
    assert!(quads_of(&p, 1).is_empty());
}
