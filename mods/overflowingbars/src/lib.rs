// Overflowing Bars (Fuzs, 1.20.1 build 8.0.1) for Horizon: the rules, without any game code.
// Source of every rule: surse/overflowingbars (branch 1.20.1): `HealthBarRenderer`, `ArmorBarRenderer`,
// `BarOverlayRenderer`, `RowCountRenderer`, `ChatOffsetHelper`, `GuiOverlayHandler`, `ClientConfig`;
// the values are the pack's `config/overflowingbars-client.toml`.
// This file is pasted into the game twice: as `crate::game::overflowingbars::engine` and inside the module
// `horizon_client::hud_layout::overflowingbars` (see tools/bundle_hud.py), and it is the `horizon_overflowingbars`
// crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).

/// the pack's file `overflowingbars-client.toml`, as Rust constants (his values are the original's defaults).
pub mod config {
    pub const TOUGHNESS_ARMOR_TOUGHNESS_BAR: bool = true; // [toughness] armor_toughness_bar
    pub const TOUGHNESS_ALLOW_COUNT: bool = true; // [toughness] allow_count
    pub const TOUGHNESS_COLORIZE_FIRST_ROW: bool = false; // [toughness] colorize_first_row
    pub const TOUGHNESS_ROW_SHIFT: i64 = 0; // [toughness] toughness_bar_row_shift (no effect on Forge)
    pub const TOUGHNESS_LEFT_SIDE: bool = false; // [toughness] left_side
    pub const TOUGHNESS_ALLOW_LAYERS: bool = true; // [toughness] allow_layers
    pub const TOUGHNESS_SKIP_EMPTY: bool = true; // [toughness] skip_empty_armor_points
    pub const TOUGHNESS_INVERSE: bool = false; // [toughness] inverse_coloring
    pub const HEALTH_COLORIZE_FIRST_ROW: bool = false; // [health] colorize_first_row
    pub const HEALTH_ALLOW_LAYERS: bool = true; // [health] allow_layers
    pub const HEALTH_ALLOW_COUNT: bool = true; // [health] allow_count
    pub const HEALTH_INVERSE: bool = false; // [health] inverse_coloring
    pub const ARMOR_COLORIZE_FIRST_ROW: bool = false; // [armor] colorize_first_row
    pub const ARMOR_ALLOW_LAYERS: bool = true; // [armor] allow_layers
    pub const ARMOR_SKIP_EMPTY: bool = true; // [armor] skip_empty_armor_points
    pub const ARMOR_ALLOW_COUNT: bool = true; // [armor] allow_count
    pub const ARMOR_INVERSE: bool = false; // [armor] inverse_coloring
    pub const ROW_COUNT_COLOR: &str = "WHITE"; // [row_count] row_count_color
    pub const ROW_COUNT_FULL_ROWS_ONLY: bool = false; // [row_count] count_full_rows_only
    pub const ROW_COUNT_FORCE_FONT: bool = false; // [row_count] force_font_renderer
    pub const ROW_COUNT_ALWAYS: bool = false; // [row_count] always_render_row_count
    pub const ROW_COUNT_X: bool = true; // [row_count] row_count_x
    pub const MOVE_CHAT_ABOVE_ARMOR: bool = true; // [general] move_chat_above_armor
    pub const MOVE_EXPERIENCE_ABOVE_BAR: bool = true; // [general] move_experience_above_bar
}

/// The sprites of the mod, in the GUI atlas (`assets/overflowingbars/textures/block/...`: a native mod's texture
/// reaches the GUI atlas through the `block` directory, as Iron Chests' screens do).
pub const ICONS: &str = "overflowingbars:block/gui/icons";
pub const NUMBERS: &str = "overflowingbars:block/font/tiny_numbers";
/// Both sheets are 256 x 256.
pub const SHEET: i32 = 256;

/// `ChatFormatting` colours (name, RGB).
pub const CHAT_COLORS: [(&str, u32); 16] = [
    ("BLACK", 0x00_0000),
    ("DARK_BLUE", 0x00_00AA),
    ("DARK_GREEN", 0x00_AA00),
    ("DARK_AQUA", 0x00_AAAA),
    ("DARK_RED", 0xAA_0000),
    ("DARK_PURPLE", 0xAA_00AA),
    ("GOLD", 0xFF_AA00),
    ("GRAY", 0xAA_AAAA),
    ("DARK_GRAY", 0x55_5555),
    ("BLUE", 0x55_55FF),
    ("GREEN", 0x55_FF55),
    ("AQUA", 0x55_FFFF),
    ("RED", 0xFF_5555),
    ("LIGHT_PURPLE", 0xFF_55FF),
    ("YELLOW", 0xFF_FF55),
    ("WHITE", 0xFF_FFFF),
];

/// `ClientConfig.IconRowConfig` / `ArmorRowConfig` / `ToughnessRowConfig`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub allow_layers: bool,
    pub allow_count: bool,
    pub colorize_first_row: bool,
    pub inverse_coloring: bool,
    /// Armor and toughness only.
    pub skip_empty: bool,
}

/// `ClientConfig.RowCountConfig`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowCount {
    pub color: u32,
    pub full_rows_only: bool,
    pub force_font: bool,
    pub always: bool,
    pub x: bool,
}

/// `ClientConfig`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub health: Row,
    pub armor: Row,
    pub toughness: Row,
    pub toughness_bar: bool,
    pub toughness_left: bool,
    pub row_count: RowCount,
    pub move_chat: bool,
    pub move_experience: bool,
}

impl Config {
    /// the pack's values.
    pub fn tuned() -> Self {
        use config::*;
        Self {
            health: Row {
                allow_layers: HEALTH_ALLOW_LAYERS,
                allow_count: HEALTH_ALLOW_COUNT,
                colorize_first_row: HEALTH_COLORIZE_FIRST_ROW,
                inverse_coloring: HEALTH_INVERSE,
                skip_empty: false,
            },
            armor: Row {
                allow_layers: ARMOR_ALLOW_LAYERS,
                allow_count: ARMOR_ALLOW_COUNT,
                colorize_first_row: ARMOR_COLORIZE_FIRST_ROW,
                inverse_coloring: ARMOR_INVERSE,
                skip_empty: ARMOR_SKIP_EMPTY,
            },
            toughness: Row {
                allow_layers: TOUGHNESS_ALLOW_LAYERS,
                allow_count: TOUGHNESS_ALLOW_COUNT,
                colorize_first_row: TOUGHNESS_COLORIZE_FIRST_ROW,
                inverse_coloring: TOUGHNESS_INVERSE,
                skip_empty: TOUGHNESS_SKIP_EMPTY,
            },
            toughness_bar: TOUGHNESS_ARMOR_TOUGHNESS_BAR,
            toughness_left: TOUGHNESS_LEFT_SIDE,
            row_count: RowCount {
                color: chat_color(ROW_COUNT_COLOR).unwrap_or(0xFF_FFFF),
                full_rows_only: ROW_COUNT_FULL_ROWS_ONLY,
                force_font: ROW_COUNT_FORCE_FONT,
                always: ROW_COUNT_ALWAYS,
                x: ROW_COUNT_X,
            },
            move_chat: MOVE_CHAT_ABOVE_ARMOR,
            move_experience: MOVE_EXPERIENCE_ABOVE_BAR,
        }
    }

    /// the pack's values, then what the settings folder of the mod overrides: one file per key, named as in the
    /// toml with its section first: `health.allow_layers`, `armor.skip_empty_armor_points`,
    /// `toughness.armor_toughness_bar`, `toughness.left_side`, `row_count.row_count_color`, `row_count.row_count_x`,
    /// `general.move_chat_above_armor`... A value that does not parse keeps the pack's.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut c = Self::tuned();
        let flag = |key: &str, into: &mut bool| {
            if let Some(v) = get(key) {
                match v.trim().to_ascii_lowercase().as_str() {
                    "true" => *into = true,
                    "false" => *into = false,
                    _ => {}
                }
            }
        };
        for (section, row) in [("health", &mut c.health), ("armor", &mut c.armor), ("toughness", &mut c.toughness)] {
            flag(&format!("{section}.allow_layers"), &mut row.allow_layers);
            flag(&format!("{section}.allow_count"), &mut row.allow_count);
            flag(&format!("{section}.colorize_first_row"), &mut row.colorize_first_row);
            flag(&format!("{section}.inverse_coloring"), &mut row.inverse_coloring);
            if section != "health" {
                flag(&format!("{section}.skip_empty_armor_points"), &mut row.skip_empty);
            }
        }
        flag("toughness.armor_toughness_bar", &mut c.toughness_bar);
        flag("toughness.left_side", &mut c.toughness_left);
        flag("row_count.count_full_rows_only", &mut c.row_count.full_rows_only);
        flag("row_count.force_font_renderer", &mut c.row_count.force_font);
        flag("row_count.always_render_row_count", &mut c.row_count.always);
        flag("row_count.row_count_x", &mut c.row_count.x);
        if let Some(rgb) = get("row_count.row_count_color").and_then(|v| chat_color(&v)) {
            c.row_count.color = rgb;
        }
        flag("general.move_chat_above_armor", &mut c.move_chat);
        flag("general.move_experience_above_bar", &mut c.move_experience);
        c
    }
}

/// A `ChatFormatting` colour by name (any case).
pub fn chat_color(name: &str) -> Option<u32> {
    CHAT_COLORS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name.trim()))
        .map(|(_, rgb)| *rgb)
}

// ---------------------------------------------------------------------------------------------- what is drawn

/// `HealthBarRenderer.HeartType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeartType {
    Container,
    Normal,
    Poisoned,
    Withered,
    Absorbing,
    Frozen,
    Orange,
}

impl HeartType {
    /// The `canBlink` of the type (`CONTAINER` has its own blinking sprite).
    pub fn can_blink(self) -> bool {
        matches!(self, Self::Normal | Self::Poisoned | Self::Withered | Self::Orange)
    }
}

/// The effect that tints the hearts (`HeartType.forPlayer`'s wither, poison and frozen tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Normal,
    Poisoned,
    Withered,
    Frozen,
}

/// `HeartType.forPlayer`.
pub fn heart_type(kind: Kind, absorbing: bool, orange: bool, inverse: bool) -> HeartType {
    match kind {
        Kind::Withered => HeartType::Withered,
        Kind::Poisoned => HeartType::Poisoned,
        Kind::Frozen => HeartType::Frozen,
        Kind::Normal => {
            if orange {
                if absorbing || !inverse { HeartType::Orange } else { HeartType::Normal }
            } else if absorbing {
                HeartType::Absorbing
            } else if inverse {
                HeartType::Orange
            } else {
                HeartType::Normal
            }
        }
    }
}

/// One thing the bars draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draw {
    /// A heart of the game's own sprites (`hud/heart/...`).
    Heart { ty: HeartType, half: bool, blink: bool, hardcore: bool, x: i32, y: i32 },
    /// A 9 x 9 icon of the mod's sheet (`ICONS`) at texel `u`, `v`.
    Icon { u: i32, v: i32, x: i32, y: i32 },
    /// A 3 x 5 digit (or the `x`) of `NUMBERS` at texel `u`, `v`, tinted `color` (ARGB).
    Digit { u: i32, v: i32, x: i32, y: i32, color: u32 },
    /// Text of the game's font (`force_font_renderer`), `color` ARGB, without shadow.
    Text { text: String, x: i32, y: i32, color: u32 },
}

/// `HeartType.getX/getY` of the orange hearts in the mod's sheet.
pub fn orange_uv(half: bool, blink: bool, hardcore: bool) -> (i32, i32) {
    let i = i32::from(half) + if blink { 2 } else { 0 };
    (i * 9, if hardcore { 4 } else { 3 } * 9)
}

fn heart(ty: HeartType, blink: bool, half: bool, hardcore: bool, x: i32, y: i32) -> Draw {
    if ty == HeartType::Orange {
        let (u, v) = orange_uv(half, blink && ty.can_blink(), hardcore);
        return Draw::Icon { u, v, x, y };
    }
    // `i = canBlink && blink ? 2 : 0`; the container's blink is its own sprite.
    let blink = if ty == HeartType::Container { blink } else { blink && ty.can_blink() };
    Draw::Heart { ty, half: half && ty != HeartType::Container, blink, hardcore, x, y }
}

/// `Mth.ceil`.
fn ceil(v: f64) -> i32 {
    v.ceil() as i32
}

/// What the health bar reads of the player (the hud's own state included).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    pub health: f32,
    /// The MAX_HEALTH attribute.
    pub max_health: f32,
    pub absorption: f32,
    /// `Hud.displayHealth`.
    pub display_health: i32,
    /// The hud's blink of the health bar this frame.
    pub blink: bool,
    pub kind: Kind,
    pub regeneration: bool,
    pub hardcore: bool,
    /// `Hud.tickCount`.
    pub tick: i32,
    /// `getArmorValue()`.
    pub armor: i32,
    /// `floor(getAttributeValue(ARMOR_TOUGHNESS))`.
    pub toughness: i32,
}

/// `HealthBarRenderer.renderPlayerHealth` + `renderHearts` at `x`, `y` (the bottom row's top left). `random` answers
/// `nextInt(bound)` of the hud's random (seeded with `tickCount * 312871`).
pub fn health_bar(cfg: &Config, p: &Player, x: i32, y: i32, random: &mut dyn FnMut(i32) -> i32) -> Vec<Draw> {
    let mut out = Vec::new();
    let current_health = ceil(f64::from(p.health));
    let current_absorption = ceil(f64::from(p.absorption));
    let display_health = p.display_health;
    let max_health = p.max_health.max(display_health.max(current_health) as f32);
    let regen_offset = if p.regeneration { p.tick % ceil(f64::from(max_health.min(20.0) + 5.0)) } else { -1 };
    let normal_hearts = 10.min(ceil(f64::from(max_health) / 2.0));
    let max_absorption_hearts = 20 - normal_hearts;
    let absorption_hearts = (20 - normal_hearts).min(ceil(f64::from(current_absorption) / 2.0));
    let inverse = cfg.health.inverse_coloring;
    let colorize = cfg.health.colorize_first_row;
    for current_heart in 0..normal_hearts + absorption_hearts {
        let px = x + (current_heart % 10) * 8;
        let mut py = y - (current_heart / 10) * 10;
        if current_health + current_absorption <= 4 {
            py += random(2);
        }
        if current_heart < normal_hearts && regen_offset == current_heart {
            py -= 2;
        }
        // The black outline and background (only seen for half hearts).
        out.push(heart(HeartType::Container, p.blink, false, p.hardcore, px, py));
        if current_heart >= normal_hearts {
            let absorbed = current_heart * 2 - normal_hearts * 2;
            if absorbed < current_absorption {
                let max_absorption_health = max_absorption_hearts * 2;
                let half = absorbed + 1 == current_absorption % max_absorption_health;
                let orange = current_absorption > max_absorption_health
                    && absorbed + 1 <= (current_absorption - 1) % max_absorption_health + 1;
                if half && orange {
                    out.push(heart(heart_type(p.kind, true, false, inverse), false, false, p.hardcore, px, py));
                }
                out.push(heart(heart_type(p.kind, true, orange, inverse), false, half, p.hardcore, px, py));
            }
        }
        if p.blink && current_heart * 2 < 20.min(display_health) {
            let last = (display_health - 1) % 20 + 1;
            let half = current_heart * 2 + 1 == last;
            let orange = display_health > 20 && current_heart * 2 + 1 <= last;
            if half && orange {
                out.push(heart(heart_type(p.kind, false, false, inverse), true, false, p.hardcore, px, py));
            }
            let colour = orange || (colorize && current_heart * 2 + 1 <= last);
            out.push(heart(heart_type(p.kind, false, colour, inverse), true, half, p.hardcore, px, py));
        }
        if current_heart * 2 < 20.min(current_health) {
            let last = (current_health - 1) % 20 + 1;
            let half = current_heart * 2 + 1 == last;
            let orange = current_health > 20 && current_heart * 2 + 1 <= last;
            if half && orange {
                out.push(heart(heart_type(p.kind, false, false, inverse), false, false, p.hardcore, px, py));
            }
            let colour = orange || (colorize && current_heart * 2 + 1 <= last);
            out.push(heart(heart_type(p.kind, false, colour, inverse), false, half, p.hardcore, px, py));
        }
    }
    out
}

/// `ArmorBarRenderer.renderArmorBar(guiGraphics, posX, posY, vOffset, armorPoints, left, unmodified, config)`:
/// the armor and the toughness rows. `v_offset` = 18 armor, 9 toughness on the left, 0 toughness on the right.
pub fn armor_bar(row: &Row, x: i32, y: i32, v_offset: i32, points: i32, left: bool, unmodified: bool) -> Vec<Draw> {
    let mut out = Vec::new();
    if points <= 0 {
        return out;
    }
    let inverse = !unmodified && row.inverse_coloring;
    let skip = !unmodified && row.skip_empty;
    let mut last_row_points = 0;
    if !unmodified && (row.colorize_first_row || points > 20) {
        last_row_points = (points - 1) % 20 + 1;
    }
    for point in 0..10 {
        let start_x = x + if left { point * 8 } else { -point * 8 - 9 };
        let slot = point * 2 + 1;
        let u = if slot < last_row_points {
            Some(if inverse { 18 } else { 36 })
        } else if slot == last_row_points {
            Some(if points > 20 {
                if inverse { 54 } else { 27 }
            } else if inverse {
                9
            } else {
                45
            })
        } else if slot < points {
            Some(if inverse { 36 } else { 18 })
        } else if slot == points {
            Some(if inverse { 45 } else { 9 })
        } else if !skip && slot > points {
            Some(0)
        } else {
            None
        };
        if let Some(u) = u {
            out.push(Draw::Icon { u, v: v_offset, x: start_x, y });
        }
    }
    out
}

/// `RowCountRenderer.drawBarRowCount`: the `3x` of the rows an overflowing bar hides. `text_width` = the font's width.
pub fn row_count(
    cfg: &RowCount,
    x: i32,
    y: i32,
    value: i32,
    left: bool,
    max_row_count: i32,
    text_width: &dyn Fn(&str) -> i32,
) -> Vec<Draw> {
    let mut out = Vec::new();
    if value <= 0 || max_row_count <= 0 {
        return out;
    }
    let rows = value as f32 / max_row_count as f32;
    if !cfg.always && rows <= 1.0 {
        return out;
    }
    let number = if cfg.full_rows_only { rows.floor() as i32 } else { rows.ceil() as i32 };
    let mut x = x;
    if cfg.force_font {
        let mut text = number.to_string();
        if cfg.x {
            text.push('x');
        }
        if left {
            x -= text_width(&text);
        }
        // `drawBorderedText(.., posY + 1, ..)`: the shadow on every side, then the text.
        let y = y + 1;
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            out.push(Draw::Text { text: text.clone(), x: x + dx, y: y + dy, color: 0xFF00_0000 });
        }
        out.push(Draw::Text { text, x, y, color: 0xFF00_0000 | cfg.color });
        return out;
    }
    // The digits, the last one first.
    let mut digits: Vec<i32> = Vec::new();
    let mut n = number;
    while n > 0 {
        digits.push(n % 10);
        n /= 10;
    }
    if left {
        x -= if cfg.x { 7 } else { 3 };
    } else {
        x += 4 * digits.len() as i32;
    }
    for (i, digit) in digits.iter().enumerate() {
        bordered(&mut out, cfg.color, x - 4 * i as i32, y + 2, 5 * digit, 0);
    }
    if cfg.x {
        bordered(&mut out, cfg.color, x + 4, y + 2, 0, 7);
    }
    out
}

/// `drawBorderedSprite`: the sprite black on its four sides, then in the colour.
fn bordered(out: &mut Vec<Draw>, color: u32, x: i32, y: i32, u: i32, v: i32) {
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
        out.push(Draw::Digit { u, v, x: x + dx, y: y + dy, color: 0xFF00_0000 });
    }
    out.push(Draw::Digit { u, v, x, y, color: 0xFF00_0000 | color });
}

// ---------------------------------------------------------------------------------------------- the whole bars

/// `ChatOffsetHelper.twoHealthRows`.
pub fn two_health_rows(max_health: f32, absorption: f32) -> bool {
    absorption > 0.0 && max_health + absorption > 20.0
}

/// `ChatOffsetHelper.getChatOffsetY`: how far (in GUI pixels) the chat moves up for the extra rows.
pub fn chat_offset(cfg: &Config, max_health: f32, absorption: f32, armor: i32, toughness: i32) -> i32 {
    let mut offset = 0;
    if two_health_rows(max_health, absorption) {
        offset += 1;
    }
    if armor > 0 {
        offset += 1;
    }
    if cfg.toughness_bar && toughness > 0 && (cfg.toughness_left || offset == 0) {
        offset += 1;
    }
    offset * 10
}

/// The bars of the original's overlays, and the heights they leave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bars {
    pub draws: Vec<Draw>,
    /// `gui.leftHeight` after the health and armor bars.
    pub left_height: i32,
    /// `gui.rightHeight` after the toughness bar.
    pub right_height: i32,
}

/// `GuiOverlayHandler` + `BarOverlayRenderer`: the health, armor and toughness bars of a `w` x `h` screen. `right_height`
/// = what the game's own bars (food, air, mount) leave on the right; `vanilla_left` = what the game's own health bar
/// leaves on the left (used when the health bar's layers are off and the game draws it). `text_width` measures text
/// for the row counts.
pub fn bars(
    cfg: &Config,
    p: &Player,
    (w, h): (i32, i32),
    right_height: i32,
    vanilla_left: i32,
    text_width: &dyn Fn(&str) -> i32,
    random: &mut dyn FnMut(i32) -> i32,
) -> Bars {
    let mut draws = Vec::new();
    let mut left_height = 39;
    let mut right_height = right_height;
    // PLAYER_HEALTH.
    if cfg.health.allow_layers {
        let (x, y) = (w / 2 - 91, h - left_height);
        draws.extend(health_bar(cfg, p, x, y, random));
        if cfg.health.allow_count {
            let all_hearts = ceil(f64::from(p.health));
            draws.extend(row_count(&cfg.row_count, x - 2, y, all_hearts, true, 20, text_width));
            // `(20 - Mth.ceil(Math.min(20, allHearts) / 2.0F)) * 2`.
            let max_absorption = (20 - ceil(f64::from(all_hearts.min(20) as f32 / 2.0))) * 2;
            draws.extend(row_count(
                &cfg.row_count,
                x - 2,
                y - 10,
                ceil(f64::from(p.absorption)),
                true,
                max_absorption,
                text_width,
            ));
        }
        left_height += if two_health_rows(p.max_health, p.absorption) { 20 } else { 10 };
    } else {
        // The game draws its own health bar: the armor goes above it.
        left_height = vanilla_left;
    }
    // ARMOR_LEVEL.
    if cfg.armor.allow_layers {
        let (x, y) = (w / 2 - 91, h - left_height);
        draws.extend(armor_bar(&cfg.armor, x, y, 18, p.armor, true, false));
        if cfg.armor.allow_count {
            draws.extend(row_count(&cfg.row_count, x - 2, y, p.armor, true, 20, text_width));
        }
        if p.armor > 0 {
            left_height += 10;
        }
    } else if p.armor > 0 {
        // The game's own armor row.
        left_height += 10;
    }
    // The toughness overlay (above MOUNT_HEALTH).
    if cfg.toughness_bar {
        let left = cfg.toughness_left;
        let (x, y) = (w / 2 + if left { -91 } else { 91 }, h - if left { left_height } else { right_height });
        let unmodified = !cfg.toughness.allow_layers;
        draws.extend(armor_bar(&cfg.toughness, x, y, if left { 9 } else { 0 }, p.toughness, left, unmodified));
        if cfg.toughness.allow_count && !unmodified {
            draws.extend(row_count(
                &cfg.row_count,
                x + if left { -2 } else { 2 },
                y,
                p.toughness,
                left,
                20,
                text_width,
            ));
        }
        if p.toughness > 0 {
            if left {
                left_height += 10;
            } else {
                right_height += 10;
            }
        }
    }
    Bars { draws, left_height, right_height }
}

/// `GuiMixin.renderExperienceBar`: the level number moves up by this many pixels.
pub const EXPERIENCE_LEVEL_SHIFT: i32 = 3;
