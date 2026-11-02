use horizon_travelerstitles::*;
use std::collections::HashMap;

#[derive(Default)]
struct L(HashMap<String, String>);
impl Lang for L {
    fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}
fn lang() -> L {
    let pairs = [
        ("biome.minecraft.plains", "Plains"),
        ("biome.minecraft.forest", "Forest"),
        ("biome.minecraft.desert", "Desert"),
        ("biome.minecraft.river", "River"),
        ("biome.minecraft.the_end", "The End"),
        ("biome.minecraft.lush_caves", "Lush Caves"),
        ("biome.minecraft.stony_peaks", "Stony Peaks"),
        ("biome.minecraft.stony_peaks.color", "c0c0c0"),
        ("travelerstitles.biome.minecraft.desert", "Dry Sea"),
        ("travelerstitles.biome.minecraft.desert.color", "ffd070"),
        ("travelerstitles.minecraft.overworld", "Overworld"),
        ("travelerstitles.minecraft.the_nether", "The Nether"),
        ("travelerstitles.minecraft.the_nether.color", "750909"),
    ];
    L(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}
fn view<'a>(dimension: &'a str, biome: &'a str) -> View<'a> {
    View { loaded: true, has_skylight: dimension == "minecraft:overworld", can_see_sky: true, dimension, biome }
}
fn manager() -> Manager {
    Manager::new(Config::tuned())
}
fn width(s: &str) -> i32 {
    s.len() as i32 * 6
}

#[test]
fn tuned_values_are_the_file() {
    let c = Config::tuned();
    assert_eq!((c.biomes.fade_in, c.biomes.display, c.biomes.fade_out, c.biomes.cooldown), (10, 50, 10, 80));
    assert_eq!((c.dimensions.fade_in, c.dimensions.display, c.dimensions.fade_out), (10, 70, 20));
    assert_eq!((c.biomes.size, c.dimensions.size), (2.1, 3.0));
    assert_eq!((c.biomes.y_offset, c.dimensions.y_offset), (-33, -32));
    assert_eq!(c.biomes.recent, 5);
    assert_eq!(c.dimensions.recent, 1);
    assert_eq!(c.biome_blacklist, ["minecraft:the_end", "minecraft:river", "minecraft:frozen_river"]);
    assert!(c.dimension_blacklist.is_empty());
    assert!(c.biomes_only_surface && !c.dimensions_only_surface && c.reset_biomes_on_dimension);
    assert!(c.biomes.centered && c.biomes.shadow);
    assert_eq!(c.biomes.color, "ffffff");
}

#[test]
fn lists_parse_like_the_config() {
    assert_eq!(parse_list("[a:b, c:d]"), ["a:b", "c:d"]);
    assert!(parse_list("[]").is_empty());
    assert_eq!(parse_list("x"), ["x"]);
}

#[test]
fn description_ids_follow_make_description_id() {
    assert_eq!(description_id("biome", "minecraft:plains"), "biome.minecraft.plains");
    assert_eq!(description_id("travelerstitles", "minecraft:the_nether"), "travelerstitles.minecraft.the_nether");
    assert_eq!(description_id("biome", "terralith:cave/ice_caves"), "biome.terralith.cave.ice_caves");
    assert_eq!(description_id("travelerstitles.biome", "minecraft:desert"), "travelerstitles.biome.minecraft.desert");
}

#[test]
fn entering_the_world_shows_the_dimension_and_the_biome_titles() {
    let mut m = manager();
    let s = m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    assert_eq!(m.dimension.displayed.as_deref(), Some("Overworld"));
    assert_eq!(m.biome.displayed.as_deref(), Some("Plains"));
    assert_eq!(m.dimension.timer, 100);
    assert_eq!(m.biome.timer, 70);
    assert_eq!(m.biome.cooldown, 80);
    // Only the dimension sound: the biome one does not overlap it.
    assert_eq!(s, vec![Sound { event: SOUND_DIMENSION, volume: 1.0, pitch: 1.0 }]);
}

#[test]
fn a_biome_title_has_its_own_sound_once_no_dimension_title_runs() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    for _ in 0..100 {
        m.client_tick(false);
    }
    assert_eq!(m.dimension.timer, 0);
    let s = m.player_tick(&view("minecraft:overworld", "minecraft:forest"), &lang());
    assert_eq!(s, vec![Sound { event: SOUND_BIOME, volume: 1.0, pitch: 1.0 }]);
    assert_eq!(m.biome.displayed.as_deref(), Some("Forest"));
}

#[test]
fn the_cooldown_holds_the_next_biome_title_back() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    // 79 ticks later the cooldown (80) still runs.
    for _ in 0..79 {
        m.client_tick(false);
    }
    assert_eq!(m.biome.cooldown, 1);
    m.player_tick(&view("minecraft:overworld", "minecraft:forest"), &lang());
    assert_eq!(m.biome.displayed.as_deref(), Some("Plains"));
    m.client_tick(false);
    m.player_tick(&view("minecraft:overworld", "minecraft:forest"), &lang());
    assert_eq!(m.biome.displayed.as_deref(), Some("Forest"));
}

#[test]
fn a_recent_biome_does_not_come_back() {
    let mut m = manager();
    let l = lang();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    for _ in 0..80 {
        m.client_tick(false);
    }
    m.player_tick(&view("minecraft:overworld", "minecraft:forest"), &l);
    for _ in 0..80 {
        m.client_tick(false);
    }
    // Back to plains: in the list of recent biomes, so no title.
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    assert_eq!(m.biome.displayed.as_deref(), Some("Forest"));
    assert_eq!(m.biome.recent.len(), 2);
}

#[test]
fn the_list_of_recent_biomes_forgets_the_oldest() {
    let mut r = Renderer::new(Config::tuned().biomes);
    for b in ["a", "b", "c", "d", "e", "f"] {
        r.add_recent(b);
    }
    assert_eq!(r.recent.len(), 5);
    assert!(!r.recent_has("a") && r.recent_has("f"));
}

#[test]
fn blacklisted_biomes_are_never_shown() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:river"), &lang());
    assert_eq!(m.biome.displayed, None);
    // The End is blacklisted too, even though the dimension title shows.
    let mut m = manager();
    m.player_tick(&view("minecraft:the_end", "minecraft:the_end"), &lang());
    assert_eq!(m.biome.displayed, None);
}

#[test]
fn a_biome_without_a_name_shows_nothing() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "mymod:unnamed"), &lang());
    assert_eq!(m.biome.displayed, None);
    assert_eq!(m.biome.timer, 0);
}

#[test]
fn the_mods_own_name_wins_over_the_games() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:desert"), &lang());
    assert_eq!(m.biome.displayed.as_deref(), Some("Dry Sea"));
    // Its colour too (ffd070), once the title has faded in.
    for _ in 0..20 {
        m.client_tick(false);
    }
    let f = m.biome.frame(0.0, width).unwrap();
    assert_eq!(f.rgb, 0xFF_D070);
}

#[test]
fn a_colour_entry_of_the_game_s_name_is_used() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:stony_peaks"), &lang());
    for _ in 0..20 {
        m.client_tick(false);
    }
    assert_eq!(m.biome.frame(0.0, width).unwrap().rgb, 0xC0_C0C0);
}

#[test]
fn underground_only_cave_biomes_update() {
    let mut m = manager();
    let l = lang();
    let mut v = view("minecraft:overworld", "minecraft:plains");
    v.can_see_sky = false;
    m.player_tick(&v, &l);
    // Under a roof the biome does not update (the dimension title still shows).
    assert_eq!(m.biome.displayed, None);
    assert_eq!(m.dimension.displayed.as_deref(), Some("Overworld"));
    let mut v = view("minecraft:overworld", "minecraft:lush_caves");
    v.can_see_sky = false;
    m.player_tick(&v, &l);
    assert_eq!(m.biome.displayed.as_deref(), Some("Lush Caves"));
}

#[test]
fn a_dimension_without_sky_light_is_never_underground() {
    let mut m = manager();
    let mut v = view("minecraft:the_nether", "minecraft:plains");
    v.has_skylight = false;
    v.can_see_sky = false;
    m.player_tick(&v, &lang());
    assert_eq!(m.biome.displayed.as_deref(), Some("Plains"));
}

#[test]
fn the_dimension_title_has_its_colour_and_comes_again_on_return() {
    let mut m = manager();
    let l = lang();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    m.player_tick(&view("minecraft:the_nether", "minecraft:plains"), &l);
    assert_eq!(m.dimension.displayed.as_deref(), Some("The Nether"));
    for _ in 0..20 {
        m.client_tick(false);
    }
    assert_eq!(m.dimension.frame(0.0, width).unwrap().rgb, 0x75_0909);
    // Standing in the Nether again: it is the most recent, so no new title (timer keeps running).
    let t = m.dimension.timer;
    m.player_tick(&view("minecraft:the_nether", "minecraft:plains"), &l);
    assert_eq!(m.dimension.timer, t);
    // Back in the Overworld: the list of 1 forgot it.
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    assert_eq!(m.dimension.displayed.as_deref(), Some("Overworld"));
}

#[test]
fn an_unnamed_dimension_shows_question_marks() {
    let mut m = manager();
    m.player_tick(&view("mymod:limbo", "minecraft:plains"), &lang());
    assert_eq!(m.dimension.displayed.as_deref(), Some("???"));
}

#[test]
fn changing_dimension_clears_the_biome_cache() {
    let mut m = manager();
    let l = lang();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    m.dimension_changed();
    assert!(m.biome.recent.is_empty());
    assert_eq!(m.biome.displayed, None);
    assert_eq!(m.biome.timer, 0);
    // The same biome, shown again at once only after the cooldown; the cooldown is not reset by the change.
    assert_eq!(m.biome.cooldown, 80);
    // With the option off the cache stays.
    let mut c = Config::tuned();
    c.reset_biomes_on_dimension = false;
    let mut m = Manager::new(c);
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    m.dimension_changed();
    assert_eq!(m.biome.recent.len(), 1);
}

#[test]
fn nothing_happens_until_the_chunk_is_loaded_and_while_paused() {
    let mut m = manager();
    let mut v = view("minecraft:overworld", "minecraft:plains");
    v.loaded = false;
    assert!(m.player_tick(&v, &lang()).is_empty());
    assert_eq!(m.dimension.displayed, None);
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    let t = m.dimension.timer;
    m.client_tick(true);
    assert_eq!(m.dimension.timer, t);
    m.client_tick(false);
    assert_eq!(m.dimension.timer, t - 1);
}

#[test]
fn disabled_titles_stay_silent() {
    let mut c = Config::tuned();
    c.biomes.enabled = false;
    c.dimensions.enabled = false;
    let mut m = Manager::new(c);
    assert!(m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang()).is_empty());
    assert_eq!(m.biome.displayed, None);
    assert_eq!(m.dimension.displayed, None);
}

#[test]
fn the_same_title_text_is_not_shown_twice_in_a_row() {
    let mut m = manager();
    let mut l = lang();
    // Two biomes with one name (a resource pack could do it).
    l.0.insert("biome.minecraft.savanna".into(), "Plains".into());
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &l);
    for _ in 0..80 {
        m.client_tick(false);
    }
    m.player_tick(&view("minecraft:overworld", "minecraft:savanna"), &l);
    assert_eq!(m.biome.cooldown, 0);
    assert!(!m.biome.recent_has("minecraft:savanna"));
}

#[test]
fn the_fade_is_the_original_s() {
    let m = manager();
    let mut r = m.biome.clone();
    r.display_title("Plains".into());
    // Timer 70 = fade in 10 + display 50 + fade out 10. First tick: nearly invisible, then fully in.
    assert_eq!(r.opacity(0.0), 0);
    r.timer = 69;
    assert_eq!(r.opacity(0.0), ((10 + 50 + 10) as f32 - 69.0) as i32 * 255 / 10);
    r.timer = 60;
    assert_eq!(r.opacity(0.0), 255);
    r.timer = 10;
    assert_eq!(r.opacity(0.0), 255);
    r.timer = 5;
    assert_eq!(r.opacity(0.0), 127);
    r.timer = 1;
    assert_eq!(r.opacity(0.0), 25);
    // At or under 8 of 255 it is not drawn at all.
    r.timer = 1;
    r.settings.fade_out = 100;
    assert!(r.frame(0.0, width).is_none());
}

#[test]
fn the_frame_is_centred_with_the_offsets() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    for _ in 0..30 {
        m.client_tick(false);
    }
    let f = m.biome.frame(0.0, width).unwrap();
    // "Plains" is 6 chars: 36 wide; -36 / 2 = -18.
    assert_eq!((f.x, f.y), (-18, -33));
    assert!(f.centered && f.shadow);
    assert_eq!(f.size, 2.1);
    assert_eq!(f.alpha, 255);
    let d = m.dimension.frame(0.0, width).unwrap();
    assert_eq!((d.x, d.y, d.size), (-(9 * 6) / 2, -32, 3.0));
    // Not centred: the offset alone.
    let mut c = Config::tuned();
    c.biomes.centered = false;
    c.biomes.x_offset = 7;
    let mut m = Manager::new(c);
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    for _ in 0..30 {
        m.client_tick(false);
    }
    assert_eq!(m.biome.frame(0.0, width).unwrap().x, 7);
}

#[test]
fn frames_are_hidden_under_the_debug_screen_and_come_dimension_first() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    for _ in 0..20 {
        m.client_tick(false);
    }
    assert!(m.frames(true, 0.0, width).is_empty());
    let f = m.frames(false, 0.0, width);
    assert_eq!(f.len(), 2);
    assert_eq!(f[0].text, "Overworld");
    assert_eq!(f[1].text, "Plains");
}

#[test]
fn a_title_ends_when_its_timer_does() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    for _ in 0..70 {
        m.client_tick(false);
    }
    assert_eq!(m.biome.timer, 0);
    assert!(m.biome.frame(0.0, width).is_none());
    // The text stays remembered: the same name is not shown again.
    assert_eq!(m.biome.displayed.as_deref(), Some("Plains"));
}

#[test]
fn the_settings_folder_overrides_tuned_values() {
    let get = |k: &str| match k {
        "biomes.textSize" => Some("3".to_string()),
        "biomes.textColor" => Some("#ff0000".to_string()),
        "biomes.biomeBlacklist" => Some("[minecraft:plains]".to_string()),
        "biomes.recentBiomeCacheSize" => Some("-4".to_string()),
        "dimensions.enabled" => Some("false".to_string()),
        "sound.biomeVolume" => Some("0.5".to_string()),
        "biomes.textDisplayTime" => Some("lots".to_string()),
        "dimensions.textColor" => Some("zzz".to_string()),
        _ => None,
    };
    let c = Config::read(get);
    assert_eq!(c.biomes.size, 3.0);
    assert_eq!(c.biomes.color, "ff0000");
    assert_eq!(c.biome_blacklist, ["minecraft:plains"]);
    assert_eq!(c.biomes.recent, 0);
    assert!(!c.dimensions.enabled);
    assert_eq!(c.biome_volume, 0.5);
    // Does not parse: the pack's.
    assert_eq!(c.biomes.display, 50);
    assert_eq!(c.dimensions.color, "ffffff");
    assert_eq!(Config::read(|_| None), Config::tuned());
}

#[test]
fn rebake_applies_new_settings_without_losing_the_state() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    let mut c = Config::tuned();
    c.biomes.size = 4.0;
    m.rebake(c);
    assert_eq!(m.biome.settings.size, 4.0);
    assert_eq!(m.biome.displayed.as_deref(), Some("Plains"));
}

#[test]
fn the_backdrop_is_the_original_s_odd_rectangle() {
    // yOffset -10: from -12 to +1, around the title's width, not at the title's own height.
    assert_eq!(backdrop(60), [-32, -12, 32, 1]);
    assert_eq!(backdrop(61), [-32, -12, 33, 1]);
}

#[test]
fn colours_multiply_per_channel() {
    assert_eq!(multiply(0xFF00_0000, 0xFFFF_FFFF), 0xFF00_0000);
    assert_eq!(multiply(0x8000_0000, 0x80FF_FFFF), 0x4000_0000 | 0);
    assert_eq!(multiply(0xFFFF_FFFF, 0x00_12_34_56), 0x00_12_34_56);
}

#[test]
fn the_biometitle_command_shows_a_title_at_once_without_sound_or_memory() {
    let mut m = manager();
    let l = lang();
    assert_eq!(m.show_biome("minecraft:forest", &l), Ok(false));
    assert_eq!(m.biome.displayed.as_deref(), Some("Forest"));
    assert_eq!(m.biome.timer, 70);
    assert_eq!(m.biome.cooldown, 80);
    assert!(m.biome.recent.is_empty());
    // A name with no namespace is minecraft's; a blacklisted biome is shown, with a warning.
    assert_eq!(m.show_biome("river", &l), Ok(true));
    assert_eq!(m.biome.displayed.as_deref(), Some("River"));
    // The mod's own name and colour.
    assert_eq!(m.show_biome("desert", &l), Ok(false));
    assert_eq!(m.biome.displayed.as_deref(), Some("Dry Sea"));
    for _ in 0..20 {
        m.client_tick(false);
    }
    assert_eq!(m.biome.frame(0.0, width).unwrap().rgb, 0xFF_D070);
}

#[test]
fn the_biometitle_command_reports_its_errors() {
    let mut m = manager();
    assert_eq!(m.show_biome("Not A Biome", &lang()), Err(CommandError::Invalid("Not A Biome".into())));
    assert_eq!(m.show_biome("mymod:nowhere", &lang()), Err(CommandError::NotFound("mymod:nowhere".into())));
    assert_eq!(m.biome.displayed, None);
}

#[test]
fn the_dimensiontitle_command_shows_the_name_and_its_colour() {
    let mut m = manager();
    assert!(!m.show_dimension("minecraft:the_nether", &lang()));
    assert_eq!(m.dimension.displayed.as_deref(), Some("The Nether"));
    assert_eq!(m.dimension.timer, 100);
    assert!(m.dimension.recent.is_empty());
    m.show_dimension("minecraft:the_end", &lang());
    assert_eq!(m.dimension.displayed.as_deref(), Some("???"));
    assert_eq!(known_dimension("overworld"), Some("minecraft:overworld"));
    assert_eq!(known_dimension("minecraft:the_end"), Some("minecraft:the_end"));
    assert_eq!(known_dimension("moon"), None);
    assert_eq!(known_dimension("Bad Name"), None);
}

#[test]
fn tt_reload_forgets_the_titles_and_takes_the_new_config() {
    let mut m = manager();
    m.player_tick(&view("minecraft:overworld", "minecraft:plains"), &lang());
    let mut c = Config::tuned();
    c.biomes.size = 5.0;
    m.reload(c);
    assert!(m.biome.recent.is_empty() && m.dimension.recent.is_empty());
    assert_eq!((m.biome.timer, m.dimension.timer), (0, 0));
    assert_eq!(m.biome.settings.size, 5.0);
    // The dimension title shows again.
    m.player_tick(&view("minecraft:overworld", "minecraft:forest"), &lang());
    assert_eq!(m.dimension.displayed.as_deref(), Some("Overworld"));
}

#[test]
fn resource_locations_are_checked() {
    assert_eq!(normalize_id("plains").as_deref(), Some("minecraft:plains"));
    assert_eq!(normalize_id("terralith:cave/ice_caves").as_deref(), Some("terralith:cave/ice_caves"));
    assert_eq!(normalize_id("UPPER"), None);
    assert_eq!(normalize_id(""), None);
    assert_eq!(normalize_id("a:"), None);
}
