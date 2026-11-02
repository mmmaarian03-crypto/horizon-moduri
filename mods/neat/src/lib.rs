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
