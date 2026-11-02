// Neat inside Horizon's game (nightly 142388e1c136). The rules and the plan of a plate are the engine
// below; this part lists the living entities near the player, answers the questions the rules ask about
// them, and puts the plate's quads, texts and icons into the world facing the camera, as Horizon puts
// its own name tags: into the entity vertices, depth tested.
use super::{Game, ItemStack, view_vector};
use horizon_assets::item_model::DisplayContext;
use horizon_client::{
    block_entity_render::vertex_light, mesher::Vertex, scoreboard as sb,
};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    // Neat for Horizon: the rules and the plan of a health plate, without any game code.
// Written from Neat 1.20.1-41/42 (Vazkii / Violet Moon, code under the MIT licence; `HealthBarRenderer`,
// `NeatConfig`, `NeatRenderType`, `EntityRendererMixin`, and the 32 x 32 `health_bar_texture.png` of
// the 1.20 branch and the jar, read for this port).
// The original adds quads to the world as each entity is rendered, facing the camera, in "plate" units
// (one unit is 0.0267 blocks, x to the right and y downwards); here `plate` lists the quads, texts and
// icons of one plate in those units and the game puts them in the world.

// ---- the config ----

/// `neat-client.toml`. The defaults are the original's, and also the values in the pack's file.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub max_distance: i32,
    pub max_distance_without_line_of_sight: i32,
    pub render_without_gui: bool,
    pub height_above_mob: f64,
    pub draw_background: bool,
    pub background_padding: i32,
    pub background_height: i32,
    pub health_bar_height: i32,
    pub plate_size: i32,
    /// Read, but the original never uses it.
    pub plate_size_boss: i32,
    pub show_attributes: bool,
    pub show_armor: bool,
    pub group_armor: bool,
    pub color_health_bar_by_type: bool,
    pub text_color: String,
    pub hp_text_height: i32,
    pub show_max_hp: bool,
    pub show_current_hp: bool,
    pub show_hp_percentage: bool,
    pub show_on_passive: bool,
    pub show_on_hostile: bool,
    pub display_on_players: bool,
    pub display_on_bosses: bool,
    pub only_health_bar_for_target: bool,
    pub show_entity_full_health: bool,
    pub show_debug_with_f3: bool,
    pub show_entity_name: bool,
    /// Not done: see the README.
    pub disable_name_tag: bool,
    pub disable_name_tag_if_healthbar: bool,
    pub icon_offset_x: f64,
    pub icon_offset_y: f64,
    pub decimal_format: String,
    pub blacklist: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_distance: 24,
            max_distance_without_line_of_sight: 8,
            render_without_gui: false,
            height_above_mob: 0.6,
            draw_background: true,
            background_padding: 2,
            background_height: 6,
            health_bar_height: 4,
            plate_size: 25,
            plate_size_boss: 50,
            show_attributes: true,
            show_armor: true,
            group_armor: true,
            color_health_bar_by_type: false,
            text_color: "FFFFFF".to_owned(),
            hp_text_height: 14,
            show_max_hp: true,
            show_current_hp: true,
            show_hp_percentage: true,
            show_on_passive: true,
            show_on_hostile: true,
            display_on_players: true,
            display_on_bosses: true,
            only_health_bar_for_target: false,
            show_entity_full_health: true,
            show_debug_with_f3: true,
            show_entity_name: true,
            disable_name_tag: false,
            disable_name_tag_if_healthbar: true,
            icon_offset_x: 0.0,
            icon_offset_y: 0.0,
            decimal_format: "#.##".to_owned(),
            blacklist: DEFAULT_DISABLED.iter().map(|s| (*s).to_owned()).collect(),
        }
    }
}

/// `NeatConfig.DEFAULT_DISABLED`.
pub const DEFAULT_DISABLED: [&str; 7] = [
    "minecraft:shulker",
    "minecraft:armor_stand",
    "minecraft:cod",
    "minecraft:salmon",
    "minecraft:pufferfish",
    "minecraft:tropical_fish",
    "minecraft:tadpole",
];

impl Config {
    /// The values stored under the toml's names; what is missing or unreadable is the default.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let d = Self::default();
        let text = |key: &str| get(key).map(|v| v.trim().trim_matches('"').to_owned());
        let flag = |key: &str, default: bool| match text(key).as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        let int = |key: &str, default: i32| text(key).and_then(|v| v.parse().ok()).unwrap_or(default);
        let float = |key: &str, default: f64| text(key).and_then(|v| v.parse().ok()).unwrap_or(default);
        Self {
            max_distance: int("max_distance", d.max_distance),
            max_distance_without_line_of_sight: int(
                "max_distance_without_line_of_sight",
                d.max_distance_without_line_of_sight,
            ),
            render_without_gui: flag("render_without_gui", d.render_without_gui),
            height_above_mob: float("height_above_mob", d.height_above_mob),
            draw_background: flag("draw_background", d.draw_background),
            background_padding: int("background_padding", d.background_padding),
            background_height: int("background_height", d.background_height),
            health_bar_height: int("health_bar_height", d.health_bar_height),
            plate_size: int("plate_size", d.plate_size),
            plate_size_boss: int("plate_size_boss", d.plate_size_boss),
            show_attributes: flag("show_attributes", d.show_attributes),
            show_armor: flag("show_armor", d.show_armor),
            group_armor: flag("group_armor", d.group_armor),
            color_health_bar_by_type: flag("color_health_bar_by_type", d.color_health_bar_by_type),
            text_color: text("text_color").unwrap_or(d.text_color),
            hp_text_height: int("hp_text_height", d.hp_text_height),
            show_max_hp: flag("show_max_hp", d.show_max_hp),
            show_current_hp: flag("show_current_hp", d.show_current_hp),
            show_hp_percentage: flag("show_hp_percentage", d.show_hp_percentage),
            show_on_passive: flag("show_on_passive", d.show_on_passive),
            show_on_hostile: flag("show_on_hostile", d.show_on_hostile),
            display_on_players: flag("display_on_players", d.display_on_players),
            display_on_bosses: flag("display_on_bosses", d.display_on_bosses),
            only_health_bar_for_target: flag("only_health_bar_for_target", d.only_health_bar_for_target),
            show_entity_full_health: flag("show_entity_full_health", d.show_entity_full_health),
            show_debug_with_f3: flag("show_debug_with_f3", d.show_debug_with_f3),
            show_entity_name: flag("show_entity_name", d.show_entity_name),
            disable_name_tag: flag("disable_name_tag", d.disable_name_tag),
            disable_name_tag_if_healthbar: flag("disable_name_tag_if_healthbar", d.disable_name_tag_if_healthbar),
            icon_offset_x: float("icon_offset_x", d.icon_offset_x),
            icon_offset_y: float("icon_offset_y", d.icon_offset_y),
            decimal_format: text("decimal_format").unwrap_or(d.decimal_format),
            blacklist: get("blacklist").map(|v| parse_list(&v)).unwrap_or(d.blacklist),
        }
    }
}

/// A toml array of strings: `["a", "b"]`.
fn parse_list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

// ---- what a mob is ----

/// 1.20.1's `MobType` (the 26.x game dropped it for entity type tags).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobKind {
    Undefined,
    Arthropod,
    Undead,
    Illager,
}

/// `LivingEntity.getMobType` by entity type: `MobType.ARTHROPOD` for the spiders, bees, silverfish and
/// endermites; `UNDEAD` for the zombies, skeletons, wither, phantom, zoglin and the undead horses;
/// `ILLAGER` for the illagers (not the ravager or the witch). The mobs 26.x added to those families
/// (the bogged, parched, camel husk and zombie nautilus) are listed with them.
pub fn mob_kind(id: &str) -> MobKind {
    match id.trim_start_matches("minecraft:") {
        "spider" | "cave_spider" | "bee" | "silverfish" | "endermite" => MobKind::Arthropod,
        "zombie" | "husk" | "drowned" | "zombie_villager" | "zombified_piglin" | "skeleton" | "stray"
        | "wither_skeleton" | "wither" | "phantom" | "skeleton_horse" | "zombie_horse" | "zoglin"
        | "bogged" | "parched" | "camel_husk" | "zombie_nautilus" => MobKind::Undead,
        "pillager" | "vindicator" | "evoker" | "illusioner" => MobKind::Illager,
        _ => MobKind::Undefined,
    }
}

/// `forge:bosses` (and `c:bosses`): the ender dragon and the wither.
pub fn is_boss(id: &str) -> bool {
    matches!(id.trim_start_matches("minecraft:"), "ender_dragon" | "wither")
}

/// `MobCategory.isFriendly`: everything but monsters and `misc` (the player's category).
pub fn is_friendly(category: &str) -> bool {
    matches!(
        category,
        "creature"
            | "ambient"
            | "axolotls"
            | "underground_water_creature"
            | "water_creature"
            | "water_ambient"
    )
}

/// The living entity a plate is for, as the game reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    /// `minecraft:zombie`.
    pub id: String,
    pub player: bool,
    /// `entity.getType().getCategory().isFriendly()`.
    pub friendly: bool,
    /// `instanceof Monster`, for the colour by type.
    pub monster: bool,
    pub health: f32,
    pub max_health: f32,
    /// `getArmorValue()`.
    pub armor: i32,
    /// The name as shown: the custom name, or the display name.
    pub name: String,
    pub custom_name: bool,
    /// `distanceTo(camera entity)`.
    pub distance: f32,
    /// `hasLineOfSight(camera entity)`; only asked beyond `max_distance_without_line_of_sight`.
    pub line_of_sight: bool,
    /// The entity the camera entity is looking at.
    pub looked_at: bool,
    /// `living.hasPassenger(cameraEntity)`.
    pub carries_camera: bool,
    /// The invisibility and team rules left it visible (`shouldShowPlate`'s last block).
    pub visible: bool,
}

/// What the plate rules read from the game besides the subject.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// `Minecraft.renderNames()`: the HUD is not hidden with F1.
    pub render_names: bool,
    /// `NeatConfig.draw`, the toggle key.
    pub drawing: bool,
    /// `mc.options.renderDebug`.
    pub debug: bool,
}

/// `HealthBarRenderer.shouldShowPlate`, after the checks that the entity is the camera's own.
pub fn should_show(cfg: &Config, env: Env, s: &Subject) -> bool {
    if (!cfg.render_without_gui && !env.render_names) || !env.drawing {
        return false;
    }
    if cfg.blacklist.iter().any(|b| *b == s.id) {
        return false;
    }
    let boss = is_boss(&s.id);
    let distance = s.distance;
    if distance > cfg.max_distance as f32
        || (distance > cfg.max_distance_without_line_of_sight as f32 && !s.line_of_sight)
    {
        return false;
    }
    if !cfg.display_on_bosses && boss {
        return false;
    }
    if !cfg.display_on_players && s.player {
        return false;
    }
    if !cfg.show_entity_full_health && s.health >= s.max_health {
        return false;
    }
    if cfg.only_health_bar_for_target && !s.looked_at {
        return false;
    }
    if !cfg.show_on_passive && s.friendly {
        return false;
    }
    if !cfg.show_on_hostile && (!s.friendly && !boss) {
        return false;
    }
    if s.carries_camera {
        return false;
    }
    s.visible
}

// ---- the plate ----

/// One plate unit in blocks (`globalScale`).
pub const GLOBAL_SCALE: f32 = 0.0267;
/// The name's scale, and the health values' (`0.75 * textScale`).
pub const TEXT_SCALE: f32 = 0.5;
pub const HEALTH_TEXT_SCALE: f32 = 0.75 * TEXT_SCALE;

/// `Mth.hsvToRgb`.
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> u32 {
    let i = ((hue * 6.0) as i32) % 6;
    let f = hue * 6.0 - i as f32;
    let f1 = value * (1.0 - saturation);
    let f2 = value * (1.0 - f * saturation);
    let f3 = value * (1.0 - (1.0 - f) * saturation);
    let (r, g, b) = match i {
        0 => (value, f3, f1),
        1 => (f2, value, f1),
        2 => (f1, value, f3),
        3 => (f1, f2, value),
        4 => (f3, f1, value),
        5 => (value, f1, f2),
        _ => (0.0, 0.0, 0.0),
    };
    let byte = |v: f32| ((v * 255.0) as i32).clamp(0, 255) as u32;
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

/// `HealthBarRenderer.getColor`: the bar's `0xRRGGBB`.
pub fn bar_color(cfg: &Config, s: &Subject) -> u32 {
    if cfg.color_health_bar_by_type {
        // Green, purple for a boss, red for a monster.
        return if is_boss(&s.id) {
            0x80_0080
        } else if s.monster {
            0xFF_0000
        } else {
            0x00_FF00
        };
    }
    let health = s.health.clamp(0.0, s.max_health.max(0.0));
    let hue = ((health / s.max_health) / 3.0 - 0.07).max(0.0);
    hsv_to_rgb(hue, 1.0, 1.0)
}
/// `java.text.DecimalFormat.format(double)` for the patterns of `0`, `#` and one `.` (HALF_EVEN, as
/// Java's default): `"#.##"` is two decimals at most, no leading zero before a fraction.
pub fn format_decimal(value: f64, pattern: &str) -> String {
    let pattern = pattern.split(';').next().unwrap_or("");
    let (int_part, frac_part) = pattern.split_once('.').unwrap_or((pattern, ""));
    let min_int = int_part.chars().filter(|c| *c == '0').count();
    let max_frac = frac_part.chars().filter(|c| *c == '0' || *c == '#').count();
    let min_frac = frac_part.chars().filter(|c| *c == '0').count();
    if value.is_nan() {
        return "NaN".to_owned();
    }
    let negative = value.is_sign_negative() && value != 0.0;
    if value.is_infinite() {
        return if negative { "-∞".to_owned() } else { "∞".to_owned() };
    }
    // The exact decimal expansion of the double.
    let exact = format!("{:.80}", value.abs());
    let (digits, fraction) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut kept: Vec<u8> = digits.bytes().chain(fraction.bytes().take(max_frac)).map(|b| b - b'0').collect();
    let int_len = digits.len();
    let rest: Vec<u8> = fraction.bytes().skip(max_frac).collect();
    let up = match rest.first() {
        Some(&d) if d > b'5' => true,
        Some(&d) if d == b'5' => {
            // A tie only when all the digits after the five are zero: then to the even digit.
            if rest[1..].iter().any(|&b| b != b'0') {
                true
            } else {
                kept.last().is_some_and(|d| d % 2 == 1)
            }
        }
        _ => false,
    };
    let mut int_len = int_len;
    if up {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, 1);
                int_len += 1;
                break;
            }
            i -= 1;
            if kept[i] == 9 {
                kept[i] = 0;
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let (int_digits, frac_digits) = kept.split_at(int_len);
    let mut int_text: String = int_digits.iter().map(|d| char::from(b'0' + d)).collect();
    // Leading zeros are not printed beyond the pattern's minimum.
    let trimmed = int_text.trim_start_matches('0').to_owned();
    int_text = if trimmed.len() < min_int {
        format!("{}{}", "0".repeat(min_int - trimmed.len()), trimmed)
    } else {
        trimmed
    };
    let mut frac_text: String = frac_digits.iter().map(|d| char::from(b'0' + d)).collect();
    while frac_text.len() > min_frac && frac_text.ends_with('0') {
        frac_text.pop();
    }
    let mut out = String::new();
    if negative && (int_text.chars().any(|c| c != '0') || frac_text.chars().any(|c| c != '0')) {
        out.push('-');
    }
    if int_text.is_empty() && frac_text.is_empty() {
        out.push('0');
    } else {
        out.push_str(&int_text);
        if !frac_text.is_empty() {
            out.push('.');
            out.push_str(&frac_text);
        }
    }
    out
}

/// A flat quad of the plate: corners in plate units, an `[r, g, b, a]` colour and how far behind the text it
/// lies (0 the text, 1 the bar, 2 the background).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlateQuad {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub rgba: [u8; 4],
    pub depth: u8,
}

/// A line of text: `x` and `y` are the top left in plate units, `scale` the font's.
#[derive(Clone, Debug, PartialEq)]
pub struct PlateText {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub bold: bool,
    pub italic: bool,
    pub rgb: u32,
}

/// An item model drawn flat on the plate: where it goes from the plate's origin, in blocks along the local axes
/// `Camera.rotation()` gives: x to the camera's left, y up, z forward (so an icon at a negative x lies to the
/// right and one at a negative z is nearer the camera).
#[derive(Clone, Debug, PartialEq)]
pub struct PlateIcon {
    pub item: &'static str,
    pub offset: [f32; 3],
}

/// `renderStatic`'s scale for the icons.
pub const ICON_SCALE: f32 = 0.12;

#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub half_size: f32,
    pub quads: Vec<PlateQuad>,
    pub texts: Vec<PlateText>,
    pub icons: Vec<PlateIcon>,
}

/// `getIcon`: the attribute icon of a mob.
fn attribute_icon(s: &Subject) -> Option<&'static str> {
    if is_boss(&s.id) {
        return Some("minecraft:nether_star");
    }
    match mob_kind(&s.id) {
        MobKind::Arthropod => Some("minecraft:spider_eye"),
        MobKind::Undead => Some("minecraft:rotten_flesh"),
        MobKind::Illager => Some("minecraft:iron_axe"),
        MobKind::Undefined => None,
    }
}

/// `HealthBarRenderer.hookRender` for one entity: the plate. `text_width` is `Font.width` of a plain string.
pub fn plate(cfg: &Config, env: Env, s: &Subject, text_width: &dyn Fn(&str) -> f32) -> Plate {
    let bar_height = cfg.health_bar_height as f32;
    let boss = is_boss(&s.id);
    let name_len = text_width(&s.name) * TEXT_SCALE;
    let half = (cfg.plate_size as f32).max(name_len / 2.0 + 10.0);
    let mut quads = Vec::new();
    let mut texts = Vec::new();
    let mut icons = Vec::new();

    // Background: black at 64/255 over the texture's black band.
    if cfg.draw_background {
        let padding = cfg.background_padding as f32;
        let mut bg_height = cfg.background_height as f32;
        if !cfg.show_entity_name {
            bg_height -= 4.0;
        }
        quads.push(PlateQuad {
            x0: -half - padding,
            y0: -bg_height,
            x1: half + padding,
            y1: bar_height + padding,
            rgba: [0, 0, 0, 64],
            depth: 2,
        });
    }

    // The health bar: the colour at 127/255 (the white band), and the empty part black at 127 (the gray band).
    let argb = bar_color(cfg, s);
    let (r, g, b) = ((argb >> 16) as u8, (argb >> 8) as u8, argb as u8);
    // There are scenarios in vanilla where the current health can temporarily exceed the max health.
    let max_health = s.health.max(s.max_health);
    let health_half = half * (s.health / max_health);
    if health_half.is_finite() {
        quads.push(PlateQuad {
            x0: -half,
            y0: 0.0,
            x1: -half + 2.0 * health_half,
            y1: bar_height,
            rgba: [r, g, b, 127],
            depth: 1,
        });
        if health_half < half {
            quads.push(PlateQuad {
                x0: -half + 2.0 * health_half,
                y0: 0.0,
                x1: half,
                y1: bar_height,
                rgba: [0, 0, 0, 127],
                depth: 1,
            });
        }
    }

    // The text, white by default and without a shadow.
    let rgb = u32::from_str_radix(cfg.text_color.trim(), 16).map_or(0xFF_FFFF, |v| v & 0xFF_FFFF);
    if cfg.show_entity_name {
        texts.push(PlateText {
            text: s.name.clone(),
            x: -half,
            y: -4.5,
            scale: TEXT_SCALE,
            bold: false,
            italic: s.custom_name,
            rgb,
        });
    }
    {
        // The health values: translate(-halfSize, -4.5) and scale 0.375, then positions in font units.
        let at = |x: f32, y: f32| (-half + x * HEALTH_TEXT_SCALE, -4.5 + y * HEALTH_TEXT_SCALE);
        let h = cfg.hp_text_height as f32;
        let bold_width = |t: &str| text_width(t) + t.chars().count() as f32;
        if cfg.show_current_hp {
            let (x, y) = at(2.0, h);
            texts.push(PlateText {
                text: format_decimal(f64::from(s.health), &cfg.decimal_format),
                x,
                y,
                scale: HEALTH_TEXT_SCALE,
                bold: false,
                italic: false,
                rgb,
            });
        }
        if cfg.show_max_hp {
            let text = format_decimal(f64::from(s.max_health), &cfg.decimal_format);
            let x = (half / HEALTH_TEXT_SCALE * 2.0) as i32 as f32 - bold_width(&text) - 2.0;
            let (x, y) = at(x, h);
            texts.push(PlateText { text, x, y, scale: HEALTH_TEXT_SCALE, bold: true, italic: false, rgb });
        }
        if cfg.show_hp_percentage {
            let text = format!("{}%", (100.0 * s.health / s.max_health) as i32);
            let x = (half / HEALTH_TEXT_SCALE) as i32 as f32 - text_width(&text) / 2.0;
            let (x, y) = at(x, h);
            texts.push(PlateText { text, x, y, scale: HEALTH_TEXT_SCALE, bold: false, italic: false, rgb });
        }
        if cfg.show_debug_with_f3 && env.debug {
            let (x, y) = at(0.0, h + 16.0);
            texts.push(PlateText {
                text: format!("ID: \"{}\"", s.id),
                x,
                y,
                scale: HEALTH_TEXT_SCALE,
                bold: false,
                italic: false,
                rgb,
            });
        }
    }

    // The icons: not scaled with the plate, so the units are multiplied in.
    {
        let z_bump = -0.1_f32;
        let mut icon_offset = 2.85_f32;
        let mut z_shift = 0.0_f32;
        let mut put = |item: &'static str, icon_offset: f32, z_shift: f32| {
            let dx = (half - icon_offset) * GLOBAL_SCALE + cfg.icon_offset_x as f32;
            let dy = 3.0 * GLOBAL_SCALE;
            let dz = z_shift * GLOBAL_SCALE;
            icons.push(PlateIcon { item, offset: [-dx, dy + cfg.icon_offset_y as f32, dz] });
        };
        if cfg.show_attributes {
            if let Some(item) = attribute_icon(s) {
                put(item, icon_offset, z_shift);
            }
            icon_offset += 5.0;
            z_shift += z_bump;
        }
        let armor = s.armor;
        if armor > 0 && cfg.show_armor {
            let (mut iron, mut diamond) = (armor % 5, armor / 5);
            if !cfg.group_armor {
                iron = armor;
                diamond = 0;
            }
            for _ in 0..iron {
                put("minecraft:iron_chestplate", icon_offset, z_shift);
                icon_offset += 1.0;
                z_shift += z_bump;
            }
            for _ in 0..diamond {
                put("minecraft:diamond_chestplate", icon_offset, z_shift);
                icon_offset += 1.0;
                z_shift += z_bump;
            }
        }
    }
    let _ = boss;
    Plate { half_size: half, quads, texts, icons }
}

}

pub const ID: &str = "neat";
pub const TITLE: &str = "Neat 1.20.1-41";
pub const DESCRIPTION: &str = "A health plate above each mob: its name, a bar coloured by its health, the numbers, an icon for its kind and its armour. The original's defaults and the pack's file.";

struct State {
    config: Option<engine::Config>,
    /// `NeatConfig.draw`: the toggle key.
    drawing: bool,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { config: None, drawing: true });
}

/// A plate vertex's light: `0xF000F0`, full brightness.
const LIGHT: u8 = 0xFF;

impl Game<'_> {
    /// `neat-client.toml`, read once (the pack's values unless a file in the mod's folder says otherwise).
    fn neat_config(&self) -> engine::Config {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.config
                .get_or_insert_with(|| engine::Config::read(|key| self.native_setting(ID, key)))
                .clone()
        })
    }

    /// The toggle key (`neat.keybind.toggle`, unbound by default).
    pub(super) fn neat_key(&self, key: &str, pressed: bool) {
        if !pressed || !self.native_on(ID) || !self.native_playing() {
            return;
        }
        if key == self.native_mod_key(ID, "toggle", "key.keyboard.unknown") {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.drawing = !s.drawing;
            });
        }
    }

    /// The mob under the crosshair, up to 32 blocks and in front of the first block (`getEntityLookedAt`).
    fn neat_target(&self) -> Option<usize> {
        let eye = self.eye_position()?;
        let dir = view_vector(self.yaw, self.pitch);
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            if !m.alive() || Some(m.id) == self.riding {
                continue;
            }
            if let Some(t) = super::ray_box(eye, dir, 32.0, m.bounds())
                && best.is_none_or(|b| t < b.1)
            {
                best = Some((i, t));
            }
        }
        let (i, distance) = best?;
        self.before_block(distance).then_some(i)
    }

    /// The plates of the living entities near the player, into the entity vertices of the frame.
    pub(super) fn neat_plates(&self, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
        if !self.native_on(ID) || !self.native_window() {
            return;
        }
        let Some(player) = self.player.as_ref() else {
            return;
        };
        let Some(eye) = self.eye_position() else {
            return;
        };
        let cfg = self.neat_config();
        let env = engine::Env {
            render_names: !self.ui.hide_gui,
            drawing: STATE.with(|s| s.borrow().drawing),
            debug: self.debug,
        };
        if !env.drawing || (!cfg.render_without_gui && !env.render_names) {
            return;
        }
        let atlas = &self.gui.atlas;
        let text_width = |text: &str| atlas.font.width(text);
        let viewer_team = self.player_team();
        let player_names: std::collections::HashMap<u32, String> =
            self.remote_player_names().into_iter().collect();
        let target = if cfg.only_health_bar_for_target {
            self.neat_target()
        } else {
            None
        };
        // The world the line of sight is asked in: built when the first mob needs it.
        let mut world = None;
        let basis = self.neat_basis();
        for (index, m) in self.mobs.iter().enumerate() {
            if !m.alive() || m.removed {
                continue;
            }
            // living == cameraEntity never happens (the player is not in the list), and a mount that
            // carries the camera has no plate (hasPassenger).
            let carries_camera = Some(m.id) == self.riding;
            let distance = {
                let (dx, dy, dz) = (
                    m.position.x - player.position.x,
                    m.position.y - player.position.y,
                    m.position.z - player.position.z,
                );
                ((dx * dx + dy * dy + dz * dz) as f32).sqrt()
            };
            if distance > cfg.max_distance as f32 {
                continue;
            }
            let id = m.kind.id.clone();
            let is_player = id == "minecraft:player";
            let own_team = if is_player {
                player_names
                    .get(&m.id)
                    .and_then(|name| self.score.board.players_team(name))
            } else {
                self.mob_team(m.id)
            };
            let invisible = sb::invisible_to(
                own_team,
                viewer_team,
                self.spectator(),
                m.invisible || m.has_effect("minecraft:invisibility"),
            );
            // An invisible mob still shows what it wears or holds.
            let wearing = [
                &m.extra.main_hand,
                &m.extra.off_hand,
                &m.extra.head,
                &m.extra.chest,
                &m.extra.legs,
                &m.extra.feet,
            ]
            .iter()
            .any(|slot| slot.is_some());
            let visible = !(invisible && !wearing);
            // The team's name tag rules decide for a mob on a team.
            let visible = sb::team_shows_name(own_team, viewer_team, visible).unwrap_or(visible);
            let line_of_sight = distance <= cfg.max_distance_without_line_of_sight as f32 || {
                let world = world.get_or_insert_with(|| self.mob_world_base(&[]));
                let from = horizon_core::geometry::Vec3::new(m.position.x, m.eye_y(), m.position.z);
                let to = horizon_core::geometry::Vec3::new(eye[0], eye[1], eye[2]);
                world.line_of_sight(from, to)
            };
            let name = self.neat_name(m, &id, player_names.get(&m.id));
            let armor = (m.kind.armor + m.worn_armor().0).floor() as i32;
            let subject = engine::Subject {
                player: is_player,
                friendly: engine::is_friendly(&m.kind.category),
                monster: m.kind.category == "monster",
                health: m.health,
                max_health: m.attrs.max_health,
                armor,
                custom_name: m.custom_name.is_some(),
                name,
                distance,
                line_of_sight,
                looked_at: target == Some(index),
                carries_camera,
                visible,
                id,
            };
            if !engine::should_show(&cfg, env, &subject) {
                continue;
            }
            let plate = engine::plate(&cfg, env, &subject, &text_width);
            // The plate hangs over the mob's head, at its height and 0.6 above (`height_above_mob`).
            let at = glam::DVec3::new(
                m.previous.x + (m.position.x - m.previous.x) * alpha,
                m.previous.y + (m.position.y - m.previous.y) * alpha,
                m.previous.z + (m.position.z - m.previous.z) * alpha,
            ) - camera
                + glam::DVec3::new(0.0, f64::from(m.height()) + cfg.height_above_mob, 0.0);
            self.mark_entity(out.len(), "");
            self.neat_emit(&plate, at, distance, &basis, out);
        }
    }

    /// `Camera.rotation()` as the billboard's axes: right, up and forward (turned round in the third
    /// person front view, as Horizon's name tags do).
    fn neat_basis(&self) -> (glam::DVec3, glam::DVec3, glam::DVec3) {
        let (yaw, pitch) = if self.camera_type == super::perspective::CameraType::ThirdPersonFront {
            (self.yaw + 180.0, -self.pitch)
        } else {
            (self.yaw, self.pitch)
        };
        let forward = glam::DVec3::from_array(view_vector(yaw, pitch));
        let right = forward.cross(glam::DVec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        (right, up, forward)
    }

    /// `living.hasCustomName() ? customName : getDisplayName()`.
    fn neat_name(&self, m: &horizon_client::mobs::Mob, id: &str, player: Option<&String>) -> String {
        if let Some(name) = &m.custom_name {
            return name.clone();
        }
        if let Some(name) = player {
            return name.clone();
        }
        let path = id.trim_start_matches("minecraft:");
        let fallback = path
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" ");
        self.translate(&format!("entity.minecraft.{path}"), &fallback)
    }

    /// One plate's quads, texts and icons as world vertices. `at` is the plate's origin from the camera.
    fn neat_emit(
        &self,
        plate: &engine::Plate,
        at: glam::DVec3,
        distance: f32,
        basis: &(glam::DVec3, glam::DVec3, glam::DVec3),
        out: &mut Vec<Vertex>,
    ) {
        let (right, up, forward) = *basis;
        let scale = f64::from(engine::GLOBAL_SCALE);
        // Each layer lies this much further from the camera than the one in front of it, so that the
        // depth test keeps the order the original's quads have.
        let step = 0.002 * (1.0 + f64::from(distance) / 16.0);
        let atlas = &self.gui.atlas;
        let white = atlas.font.white();
        let light = vertex_light(LIGHT);
        let color = |rgb: u32, a: u8| {
            u32::from(a) << 24 | ((rgb & 0xFF) << 16) | (rgb & 0xFF00) | ((rgb >> 16) & 0xFF)
        };
        // Four corners in plate units (bottom left, bottom right, top right, top left), a texture
        // rectangle and a layer.
        let mut quad = |c: [(f32, f32); 4], uv: [f32; 4], rgba: u32, depth: u8| {
            let uvs = [[uv[0], uv[3]], [uv[2], uv[3]], [uv[2], uv[1]], [uv[0], uv[1]]];
            let back = forward * (step * f64::from(depth));
            for ((x, y), uv) in c.into_iter().zip(uvs) {
                let p = at + right * (f64::from(x) * scale) - up * (f64::from(y) * scale) + back;
                out.push(Vertex {
                    normal: [0; 4],
                    pos: p.as_vec3().to_array(),
                    uv,
                    layer: 0,
                    color: rgba,
                    light,
                    glint_uv: [0.0, 0.0],
                });
            }
        };
        let solid = [white.u0, white.v0, white.u1, white.v1];
        for q in &plate.quads {
            let c = [(q.x0, q.y1), (q.x1, q.y1), (q.x1, q.y0), (q.x0, q.y0)];
            let rgba = color(
                u32::from(q.rgba[0]) << 16 | u32::from(q.rgba[1]) << 8 | u32::from(q.rgba[2]),
                q.rgba[3],
            );
            quad(c, solid, rgba, q.depth);
        }
        for t in &plate.texts {
            let rgba = color(t.rgb, 0xFF);
            let mut cursor = 0.0_f32;
            for ch in t.text.chars() {
                let Some((rect, advance)) = atlas.glyph(ch) else {
                    cursor += 6.0 * t.scale;
                    continue;
                };
                // A bold glyph is drawn twice, one font pixel apart, and is one pixel wider.
                let passes: &[f32] = if t.bold { &[0.0, 1.0] } else { &[0.0] };
                if ch != ' ' {
                    for extra in passes {
                        let x0 = t.x + cursor + extra * t.scale;
                        let (x1, y0, y1) = (x0 + 8.0 * t.scale, t.y, t.y + 8.0 * t.scale);
                        // Italic: the top leans right by a pixel, the bottom left by a pixel.
                        let (top, bottom) = if t.italic { (t.scale, -t.scale) } else { (0.0, 0.0) };
                        let c = [
                            (x0 + bottom, y1),
                            (x1 + bottom, y1),
                            (x1 + top, y0),
                            (x0 + top, y0),
                        ];
                        quad(c, [rect.u0, rect.v0, rect.u1, rect.v1], rgba, 0);
                    }
                }
                cursor += (advance + if t.bold { 1.0 } else { 0.0 }) * t.scale;
            }
        }
        // The icons: item models, flat, not scaled with the plate. `Camera.rotation()` takes the local x to
        // the camera's left, y up and z forward (which is why the original negates x and the plate's own
        // scale is `(-s, -s, s)`), so the icons' offsets are along those.
        let left = -right;
        for icon in &plate.icons {
            let stack = ItemStack::new(icon.item, 1);
            let state = self.resolve_item(&stack, DisplayContext::None, None, 0);
            if state.is_empty() {
                continue;
            }
            let origin = at
                + left * f64::from(icon.offset[0])
                + up * f64::from(icon.offset[1])
                + forward * f64::from(icon.offset[2]);
            let basis = glam::Mat4::from_cols(
                left.as_vec3().extend(0.0),
                up.as_vec3().extend(0.0),
                forward.as_vec3().extend(0.0),
                origin.as_vec3().extend(1.0),
            );
            // translate, scale 0.12, then Axis.YP.rotationDegrees(180).
            let pose = basis
                * glam::Mat4::from_scale(glam::Vec3::splat(engine::ICON_SCALE))
                * glam::Mat4::from_rotation_y(std::f32::consts::PI);
            self.submit_item_world(&state, pose, light, out);
        }
    }
}
