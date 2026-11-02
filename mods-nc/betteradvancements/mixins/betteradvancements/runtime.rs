// Better Advancements inside Horizon's game (nightly 142388e1c136). The rules (the window, the tabs and their paging,
// the scroll, the lines, the hover box, the criteria grid, the config) are the engine below; this part reads the
// advancement tree and the progress, draws the screen with the game's GUI sprites and answers the mouse. The
// screen replaces the game's own (draw_advancements and its mouse hooks are cancelled while the mod is on).
use super::{Game, KeyCode};
use horizon_client::{
    advancements::{Advancement, Frame, Tree},
    hud::{HudBatch, rgba},
    items::ItemStack,
    text::{Style, Text},
    ui,
};
use std::{cell::RefCell, sync::Arc};

#[allow(dead_code)]
mod engine {
    // Better Advancements for Horizon: the rules, without any game code.
// Written from Better Advancements 0.6.0.73 for Minecraft 1.20.1 (way2muchnoise, "Don't Be a Jerk"
// non-commercial licence; the `1.20.1` branch cloned in `surse\betteradvancements`, read for this port:
// `BetterAdvancementsScreen`, `BetterAdvancementTab`, `BetterAdvancementTabType`, `BetterAdvancementWidget`,
// `BetterDisplayInfo`, `CriterionGrid`, `CriterionTranslator`, `CriteriaDetail`, `ColorHelper`,
// `AdvancementComparer` and the Forge `ConfigValues`/`GuiOpenHandler`; the numbers below are theirs and the
// comments name the method). The game gives the tree, the progress and the font; the drawing is in the
// adapter. Everything here is in GUI pixels as integers, like the original's.
use std::cmp::Ordering;
use std::collections::HashMap;

// ---------------------------------------------------------------------------------------------------
// Constants of `BetterAdvancementsScreen` and `BetterAdvancementWidget`.
pub const SIDE: i32 = 30;
pub const TOP: i32 = 40;
pub const BOTTOM: i32 = 30;
pub const PADDING: i32 = 9;
/// `WIDTH`, `HEIGHT` and `CORNER_SIZE` of the screen: the window texture's size and the corner it is cut by.
pub const WINDOW_TEXTURE_W: i32 = 252;
pub const WINDOW_TEXTURE_H: i32 = 140;
pub const CORNER: i32 = 30;
/// `BetterAdvancementWidget.ADVANCEMENT_SIZE`.
pub const ADVANCEMENT_SIZE: i32 = 26;
/// `WIDGET_WIDTH`, `WIDGET_HEIGHT`, `TITLE_SIZE`.
pub const WIDGET_WIDTH: i32 = 256;
pub const WIDGET_HEIGHT: i32 = 26;
pub const TITLE_SIZE: i32 = 32;
/// `Font.lineHeight`.
pub const LINE_HEIGHT: i32 = 9;

pub const WHITE: u32 = 0xFFFF_FFFF;
const BORDER: u32 = 0xFF00_0000;

// ---------------------------------------------------------------------------------------------------
// Colours (`ColorHelper`).

/// `ColorHelper.RGB(String)`: `#RRGGBB` and nothing else, as `0xFFRRGGBB`.
pub fn rgb_from_str(s: &str) -> Option<u32> {
    let s = s.trim();
    if !s.starts_with('#') || s.chars().count() != 7 {
        return None;
    }
    let hex = &s[1..];
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(hex, 16).ok().map(|v| 0xFF00_0000 | v)
}

// ---------------------------------------------------------------------------------------------------
// The config (`ConfigValues`, the Forge file `betteradvancements-client.toml`).

/// `CriteriaDetail`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    Off,
    Default,
    Spoiler,
    All,
}
impl Detail {
    /// `CriteriaDetail.fromName`: the exact name, anything else `DEFAULT`.
    pub fn from_name(name: &str) -> Detail {
        match name {
            "Off" => Detail::Off,
            "Default" => Detail::Default,
            "Spoiler" => Detail::Spoiler,
            "All" => Detail::All,
            _ => Detail::Default,
        }
    }
    pub fn show_obtained(self) -> bool {
        matches!(self, Detail::Default | Detail::All)
    }
    pub fn show_unobtained(self) -> bool {
        matches!(self, Detail::Spoiler | Detail::All)
    }
}

/// The client config. The defaults are the original's and, key for key, the values of the pack's
/// `betteradvancements-client.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub uncompleted_icon_color: u32,
    pub uncompleted_title_color: u32,
    pub completed_icon_color: u32,
    pub completed_title_color: u32,
    pub do_fade: bool,
    pub show_debug_coordinates: bool,
    pub order_tabs_alphabetically: bool,
    pub ui_scaling: i32,
    pub detail: Detail,
    pub requires_shift: bool,
    pub color_whole_criteria_text: bool,
    pub criteria_rotation_speed: i32,
    pub add_inventory_button: bool,
    pub draw_direct_lines: bool,
    pub hide_lines: bool,
    pub completed_line_color: u32,
    pub uncompleted_line_color: u32,
    pub only_use_above_tabs: bool,
}
impl Default for Config {
    fn default() -> Self {
        Config {
            uncompleted_icon_color: MC_UNCOMPLETED_ICON,
            uncompleted_title_color: MC_UNCOMPLETED_TITLE,
            completed_icon_color: MC_COMPLETED_ICON,
            completed_title_color: MC_COMPLETED_TITLE,
            do_fade: true,
            show_debug_coordinates: false,
            order_tabs_alphabetically: false,
            ui_scaling: 100,
            detail: Detail::Default,
            requires_shift: false,
            color_whole_criteria_text: false,
            criteria_rotation_speed: 3,
            add_inventory_button: false,
            draw_direct_lines: false,
            hide_lines: false,
            completed_line_color: 0xFFFF_FFFF,
            uncompleted_line_color: 0xFFFF_FFFF,
            only_use_above_tabs: false,
        }
    }
}
impl Config {
    /// Reads each key (the file's own names); a missing or bad value is the default, as a Forge spec resets it.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Config {
        let d = Config::default();
        let text = |key: &str| {
            get(key).map(|v| {
                let v = v.trim();
                v.trim_matches('"').to_owned()
            })
        };
        let flag = |key: &str, default: bool| match text(key).as_deref() {
            Some(v) if v.eq_ignore_ascii_case("true") => true,
            Some(v) if v.eq_ignore_ascii_case("false") => false,
            _ => default,
        };
        let color = |key: &str, default: u32| {
            text(key)
                .and_then(|v| rgb_from_str(&v))
                .unwrap_or(default)
        };
        let ranged = |key: &str, min: i32, max: i32, default: i32| {
            text(key)
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|v| (min..=max).contains(v))
                .unwrap_or(default)
        };
        Config {
            uncompleted_icon_color: color("defaultUncompletedIconColor", d.uncompleted_icon_color),
            uncompleted_title_color: color("defaultUncompletedTitleColor", d.uncompleted_title_color),
            completed_icon_color: color("defaultCompletedIconColor", d.completed_icon_color),
            completed_title_color: color("defaultCompletedTitleColor", d.completed_title_color),
            do_fade: flag("doAdvancementsBackgroundFade", d.do_fade),
            show_debug_coordinates: flag("showDebugCoordinates", d.show_debug_coordinates),
            order_tabs_alphabetically: flag("orderTabsAlphabetically", d.order_tabs_alphabetically),
            ui_scaling: ranged("uiScaling", 1, 100, d.ui_scaling),
            detail: text("criteriaDetail").map_or(d.detail, |v| Detail::from_name(&v)),
            requires_shift: flag("criteriaDetailRequiresShift", d.requires_shift),
            color_whole_criteria_text: flag("colorWholeCriteriaText", d.color_whole_criteria_text),
            criteria_rotation_speed: ranged("criteriaRotationSpeed", 1, 10, d.criteria_rotation_speed),
            add_inventory_button: flag("addInventoryButton", d.add_inventory_button),
            draw_direct_lines: flag("defaultDrawDirectLines", d.draw_direct_lines),
            hide_lines: flag("defaultHideLines", d.hide_lines),
            completed_line_color: color("defaultCompletedLineColor", d.completed_line_color),
            uncompleted_line_color: color("defaultUncompletedLineColor", d.uncompleted_line_color),
            only_use_above_tabs: flag("onlyUseAboveAdvancementTabs", d.only_use_above_tabs),
        }
    }
}

// `BetterDisplayInfo.defaultMinecraft*Color`.
pub const MC_COMPLETED_ICON: u32 = 0xFFDB_A213;
pub const MC_UNCOMPLETED_ICON: u32 = 0xFFFF_FFFF;
pub const MC_COMPLETED_TITLE: u32 = 0xFFDB_A213;
pub const MC_UNCOMPLETED_TITLE: u32 = 0xFF04_89C1;

/// `BetterDisplayInfo` of one advancement. In 1.20.1 nothing fills it but the config: the loader of the
/// `better_display` JSON is never called and vanilla's `DisplayInfo` does not implement `IBetterDisplayInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub completed_icon: u32,
    pub uncompleted_icon: u32,
    pub completed_title: u32,
    pub uncompleted_title: u32,
    pub direct_lines: bool,
    pub completed_line: u32,
    pub uncompleted_line: u32,
    pub hide_lines: bool,
}
impl Look {
    pub fn from_config(c: &Config) -> Look {
        Look {
            completed_icon: c.completed_icon_color,
            uncompleted_icon: c.uncompleted_icon_color,
            completed_title: c.completed_title_color,
            uncompleted_title: c.uncompleted_title_color,
            direct_lines: c.draw_direct_lines,
            completed_line: c.completed_line_color,
            uncompleted_line: c.uncompleted_line_color,
            hide_lines: c.hide_lines,
        }
    }
    /// `hasCustomIconColor`.
    pub fn has_custom_icon_color(&self) -> bool {
        self.completed_icon != MC_COMPLETED_ICON || self.uncompleted_icon != MC_UNCOMPLETED_ICON
    }
    /// `hasCustomTitleColor`.
    pub fn has_custom_title_color(&self) -> bool {
        self.completed_title != MC_COMPLETED_TITLE || self.uncompleted_title != MC_UNCOMPLETED_TITLE
    }
    /// `getIconYMultiplier`: the row of `widgets.png` (2 is the grey, tintable one).
    pub fn icon_row(&self, obtained: bool) -> i32 {
        if self.has_custom_icon_color() {
            2
        } else if obtained {
            0
        } else {
            1
        }
    }
    /// `getIconColor`.
    pub fn icon_color(&self, obtained: bool) -> u32 {
        if !self.has_custom_icon_color() {
            WHITE
        } else if obtained {
            self.completed_icon
        } else {
            self.uncompleted_icon
        }
    }
    /// `getTitleYMultiplier`.
    pub fn title_row(&self, obtained: bool) -> i32 {
        if self.has_custom_title_color() {
            3
        } else if obtained {
            0
        } else {
            1
        }
    }
    /// `getTitleColor`: the original asks `hasCustomIconColor` here (not the title one), kept as it is.
    pub fn title_color(&self, obtained: bool) -> u32 {
        if !self.has_custom_icon_color() {
            WHITE
        } else if obtained {
            self.completed_title
        } else {
            self.uncompleted_title
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// Java helpers the original's numbers depend on.

/// `Math.round(double)`.
pub fn java_round(v: f64) -> i32 {
    (v + 0.5).floor() as i32
}
/// `Mth.clamp(double, min, max)`.
pub fn clamp_f(v: f64, min: f64, max: f64) -> f64 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}
/// `Mth.clamp(float, min, max)`.
fn clamp_f32(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}
/// `Mth.clamp(int, min, max)`.
fn clamp_i(v: i32, min: i32, max: i32) -> i32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}
/// `String.hashCode`.
pub fn java_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for unit in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    h
}
/// The iteration order of a `HashMap<String, _>` filled by `FriendlyByteBuf.readMap`
/// (`Maps.newHashMapWithExpectedSize(n)`) with `keys` put in this order: by bucket, a bucket in put order.
/// The client of 1.20.1 reads an advancement's criteria this way (`Criterion.criteriaFromNetwork`), so this is the
/// order `CriterionGrid` lists them in. Returns the indexes of `keys`.
pub fn java_map_order(keys: &[String]) -> Vec<usize> {
    let n = keys.len();
    // Guava's capacity(expectedSize), then HashMap(int): the table has tableSizeFor(capacity) buckets.
    let capacity: usize = if n < 3 { n + 1 } else { (n as f32 / 0.75 + 1.0) as usize };
    let table = capacity.max(1).next_power_of_two();
    let mut order: Vec<usize> = (0..n).collect();
    let bucket = |i: usize| {
        let h = java_hash(&keys[i]) as u32;
        ((h ^ (h >> 16)) as usize) & (table - 1)
    };
    // A stable sort by bucket keeps the put order inside a bucket.
    order.sort_by_key(|&i| bucket(i));
    order
}
/// Java's `String.split(regex)` on one character: leading empty strings stay, trailing ones go.
fn java_split(s: &str, sep: char) -> Vec<String> {
    let mut parts: Vec<String> = s.split(sep).map(str::to_owned).collect();
    if s.is_empty() {
        return vec![String::new()];
    }
    while parts.last().is_some_and(String::is_empty) {
        parts.pop();
    }
    parts
}

// ---------------------------------------------------------------------------------------------------
// The window (`BetterAdvancementsScreen.init`/`render`).

/// The screen's rectangle: the GUI size and the `uiScaling` shrink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub width: i32,
    pub height: i32,
    pub internal_width: i32,
    pub internal_height: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
impl Window {
    pub fn new(width: i32, height: i32, ui_scaling: i32) -> Window {
        let internal_height = height * ui_scaling / 100;
        let internal_width = width * ui_scaling / 100;
        let left = SIDE + (width - internal_width) / 2;
        let top = TOP + (height - internal_height) / 2;
        let right = internal_width - SIDE + (width - internal_width) / 2;
        let bottom = internal_height - SIDE + (height - internal_height) / 2;
        Window { width, height, internal_width, internal_height, left, top, right, bottom }
    }
    /// `right - left` and `bottom - top`: what `render` hands to the tab types.
    pub fn render_size(&self) -> (i32, i32) {
        (self.right - self.left, self.bottom - self.top)
    }
    /// What `mouseClicked` hands to `isMouseOver` (its height is not `render`'s unless `uiScaling` is 100).
    pub fn click_size(&self) -> (i32, i32) {
        (self.internal_width - 2 * SIDE, self.internal_height - self.top - BOTTOM)
    }
    /// What `onAddAdvancementRoot` hands to `BetterAdvancementTab.create` (the tab's type is chosen with it).
    pub fn create_size(&self) -> (i32, i32) {
        (self.internal_width - 2 * SIDE, self.internal_height - TOP - SIDE)
    }
    /// The inside box (`renderInside`).
    pub fn box_rect(&self) -> (i32, i32, i32, i32) {
        (self.left + PADDING, self.top + 2 * PADDING, self.right - PADDING, self.bottom - PADDING)
    }
    /// The box's width and height (`drawContents` centres with them).
    pub fn box_size(&self) -> (i32, i32) {
        let (l, t, r, b) = self.box_rect();
        (r - l, b - t)
    }
    /// What `scroll` and `loadScroll` are given: `internalWidth - 2*SIDE - 3*PADDING` and the height one.
    pub fn scroll_size(&self) -> (i32, i32) {
        (
            self.internal_width - 2 * SIDE - 3 * PADDING,
            self.internal_height - TOP - BOTTOM - 3 * PADDING,
        )
    }
    /// `drawToolTips`' width and height.
    pub fn tooltip_size(&self) -> (i32, i32) {
        (self.right - self.left - 2 * PADDING, self.bottom - self.top - 3 * PADDING)
    }
}

// ---------------------------------------------------------------------------------------------------
// Tabs (`BetterAdvancementTabType`).

/// The sides tabs sit on, in the order `ALL` lists them (BELOW is left out for the page buttons).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Above,
    Right,
    Left,
}
impl Side {
    /// (width, height) of the tab sprite.
    pub fn size(self) -> (i32, i32) {
        match self {
            Side::Above => (28, 32),
            Side::Right | Side::Left => (32, 28),
        }
    }
    /// `getMax(width, height)`.
    pub fn max(self, width: i32, height: i32) -> i32 {
        match self {
            Side::Left | Side::Right => height / 32,
            Side::Above => width / 32,
        }
    }
}
/// `getMaxTabs`: all three sides, or only the one above.
pub fn max_tabs(width: i32, height: i32, only_above: bool) -> i32 {
    if only_above {
        return Side::Above.max(width, height);
    }
    [Side::Above, Side::Right, Side::Left].iter().map(|s| s.max(width, height)).sum()
}
/// `getTabType`: the side of the tab with this index, none when the window holds no tab at all.
pub fn tab_side(width: i32, height: i32, index: i32, only_above: bool) -> Option<Side> {
    let per_page = max_tabs(width, height, only_above);
    if per_page <= 0 {
        return None;
    }
    let on_page = index % per_page;
    let above = Side::Above.max(width, height);
    let right = Side::Right.max(width, height);
    let left = Side::Left.max(width, height);
    if on_page < above {
        Some(Side::Above)
    } else if on_page < above + right {
        Some(Side::Right)
    } else if on_page < above + right + left {
        Some(Side::Left)
    } else {
        None
    }
}
/// `index %= getMax(width, height)` of a tab's own side (Java would divide by zero with a window too small).
fn tab_index(side: Side, index: i32, width: i32, height: i32) -> i32 {
    let m = side.max(width, height);
    if m > 0 { index % m } else { 0 }
}
/// `getX` relative to the window's left.
pub fn tab_x(side: Side, index: i32, width: i32, height: i32) -> i32 {
    let i = tab_index(side, index, width, height);
    let (tw, _) = side.size();
    match side {
        Side::Above => (tw + 4) * i,
        Side::Left => -tw + 4,
        Side::Right => width - 4,
    }
}
/// `getY` relative to the window's top.
pub fn tab_y(side: Side, index: i32, width: i32, height: i32) -> i32 {
    let i = tab_index(side, index, width, height);
    let (_, th) = side.size();
    match side {
        Side::Above => -th + 4,
        Side::Left | Side::Right => th * i,
    }
}
/// `isMouseOver` of a tab.
pub fn tab_over(side: Side, left: i32, top: i32, width: i32, height: i32, index: i32, mx: f64, my: f64) -> bool {
    let x = f64::from(left + tab_x(side, index, width, height));
    let y = f64::from(top + tab_y(side, index, width, height));
    let (tw, th) = side.size();
    mx > x && mx < x + f64::from(tw) && my > y && my < y + f64::from(th)
}
/// `draw`'s texture column: 0 first, 1 middle, 2 last (`index > 0` and `x + width == width` each add one).
pub fn tab_column(side: Side, left: i32, width: i32, height: i32, index: i32) -> i32 {
    let i = tab_index(side, index, width, height);
    let (tw, _) = side.size();
    i32::from(i > 0) + i32::from(left + tw == width)
}
/// The offset of a tab's icon inside its sprite (`drawIcon`).
pub fn tab_icon_offset(side: Side) -> (i32, i32) {
    match side {
        Side::Above => (6, 9),
        Side::Left => (10, 5),
        Side::Right => (6, 5),
    }
}

/// `tabPage` and `maxPages`, both static in the original: they outlive the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Paging {
    pub page: i32,
    pub max_pages: i32,
}
impl Paging {
    /// The end of `init`: with more tabs than fit, the page buttons exist and the page count is set.
    /// Returns whether the buttons exist.
    pub fn init(&mut self, tabs: i32, per_page: i32) -> bool {
        if per_page > 0 && tabs > per_page {
            self.max_pages = tabs / per_page;
            self.page = self.page.min(self.max_pages);
            true
        } else {
            false
        }
    }
    /// The `<` button.
    pub fn previous(&mut self) {
        self.page = (self.page - 1).max(0);
    }
    /// The `>` button.
    pub fn next(&mut self) {
        self.page = (self.page + 1).min(self.max_pages);
    }
    /// The tabs of the page: `skip(page * perPage).limit(perPage)`.
    pub fn range(&self, per_page: i32, tabs: usize) -> std::ops::Range<usize> {
        let skip = (self.page.max(0) as usize).saturating_mul(per_page.max(0) as usize);
        let start = skip.min(tabs);
        let end = start.saturating_add(per_page.max(0) as usize).min(tabs);
        start..end
    }
    /// `"%d / %d"` and its x/y (`left + (internalWidth - w) / 2 - w`, `bottom + 8`).
    pub fn label(&self, text_width: i32, w: &Window) -> (String, i32, i32) {
        (
            format!("{} / {}", self.page + 1, self.max_pages + 1),
            w.left + (w.internal_width - text_width) / 2 - text_width,
            w.bottom + 8,
        )
    }
}
/// The page buttons' rectangles `(x, y, w, h)`: `<` then `>`.
pub fn page_buttons(w: &Window) -> [(i32, i32, i32, i32); 2] {
    [(w.left, w.bottom + 4, 20, 20), (w.right - 20, w.bottom + 4, 20, 20)]
}

// ---------------------------------------------------------------------------------------------------
// A tab's scroll (`BetterAdvancementTab`).

/// The extent of the widgets of a tab (`minX`... `maxY`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}
impl Bounds {
    /// `addWidget`: each widget is 28 wide and 27 high from its position.
    pub fn of(positions: &[(i32, i32)]) -> Bounds {
        let mut b = Bounds { min_x: i32::MAX, max_x: i32::MIN, min_y: i32::MAX, max_y: i32::MIN };
        for &(x, y) in positions {
            b.min_x = b.min_x.min(x);
            b.max_x = b.max_x.max(x + 28);
            b.min_y = b.min_y.min(y);
            b.max_y = b.max_y.max(y + 27);
        }
        b
    }
    pub fn is_empty(&self) -> bool {
        self.min_x > self.max_x
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub x: i32,
    pub y: i32,
    pub centered: bool,
}
impl Scroll {
    /// `drawContents`' first frame: centred on the widgets (Java's int division, towards zero).
    pub fn center(&mut self, b: &Bounds, width: i32, height: i32) {
        if !self.centered {
            if !b.is_empty() {
                self.x = (width - (b.max_x + b.min_x)) / 2;
                self.y = (height - (b.max_y + b.min_y)) / 2;
            }
            self.centered = true;
        }
    }
    /// `scroll(dx, dy, width, height)`: only an axis that is longer than the view moves.
    pub fn by(&mut self, b: &Bounds, dx: f64, dy: f64, width: i32, height: i32) {
        if b.is_empty() {
            return;
        }
        if b.max_x - b.min_x > width {
            self.x = java_round(clamp_f(
                f64::from(self.x) + dx,
                f64::from(-(b.max_x - width)),
                f64::from(-b.min_x),
            ));
        }
        if b.max_y - b.min_y > height {
            self.y = java_round(clamp_f(
                f64::from(self.y) + dy,
                f64::from(-(b.max_y - height)),
                f64::from(-b.min_y),
            ));
        }
    }
    /// `loadScroll`: what was stored when the tab was left, if anything.
    pub fn load(&mut self, b: &Bounds, saved: Option<(i32, i32)>, width: i32, height: i32) {
        if let Some((sx, sy)) = saved {
            self.centered = true;
            self.x = sx;
            self.y = sy;
            if !b.is_empty() {
                if b.max_x - b.min_x > width {
                    self.x = clamp_i(self.x, -(b.max_x - width), -b.min_x);
                }
                if b.max_y - b.min_y > height {
                    self.y = clamp_i(self.y, -(b.max_y - height), -b.min_y);
                }
            }
        }
    }
}
/// `BetterAdvancementTab.scrollHistory`, static in the original.
#[derive(Clone, Debug, Default)]
pub struct History(pub HashMap<String, (i32, i32)>);

/// The tabs' state of one screen: the selected root, each tab's scroll and `isScrolling`.
#[derive(Clone, Debug, Default)]
pub struct Tabs {
    pub selected: Option<String>,
    scrolls: HashMap<String, Scroll>,
    /// `isScrolling`: false until the first drag step of the screen, which only sets it.
    pub scrolling: bool,
}
impl Tabs {
    /// `onSelectedTabChanged`: the old tab stores its scroll, the new one loads what was stored.
    pub fn select(&mut self, root: &str, b: &Bounds, history: &mut History, w: &Window) {
        if let Some(old) = self.selected.clone() {
            let s = self.scrolls.entry(old.clone()).or_default();
            history.0.insert(old, (s.x, s.y));
        }
        self.selected = Some(root.to_owned());
        let (sw, sh) = w.scroll_size();
        let saved = history.0.get(root).copied();
        self.scrolls.entry(root.to_owned()).or_default().load(b, saved, sw, sh);
    }
    /// The selected tab's scroll for this frame, centred on its first.
    pub fn frame_scroll(&mut self, root: &str, b: &Bounds, w: &Window) -> (i32, i32) {
        let (bw, bh) = w.box_size();
        let s = self.scrolls.entry(root.to_owned()).or_default();
        s.center(b, bw, bh);
        (s.x, s.y)
    }
    /// A wheel or drag step on the selected tab.
    pub fn scroll_by(&mut self, b: &Bounds, dx: f64, dy: f64, w: &Window) {
        let Some(root) = self.selected.clone() else { return };
        let (sw, sh) = w.scroll_size();
        self.scrolls.entry(root).or_default().by(b, dx, dy, sw, sh);
    }
    /// `mouseDragged`: false for the first step (it only starts `isScrolling`), true when the tab scrolls.
    pub fn drag_step(&mut self) -> bool {
        if self.scrolling {
            true
        } else {
            self.scrolling = true;
            false
        }
    }
    pub fn scroll_of(&self, root: &str) -> (i32, i32) {
        self.scrolls.get(root).map_or((0, 0), |s| (s.x, s.y))
    }
}

// ---------------------------------------------------------------------------------------------------
// Widget positions and the lines between them (`BetterAdvancementWidget`).

/// A widget's place: `floor(x * 32)` and `floor(y * 27)` (the original uses 32 here, vanilla 28), floats as `DisplayInfo`.
pub fn widget_pos(x: f32, y: f32) -> (i32, i32) {
    ((x * 32.0).floor() as i32, (y * 27.0).floor() as i32)
}

/// One thing a connection draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// A filled rectangle `x0..x1`, `y0..y1` (`GuiGraphics.fill`: corners in either order).
    Fill { x0: i32, y0: i32, x1: i32, y1: i32, color: u32 },
    /// `RenderUtil.drawRect`'s quad, corners in order.
    Quad { corners: [[f64; 2]; 4], color: u32 },
}
fn h_line(out: &mut Vec<Draw>, x1: i32, x2: i32, y: i32, color: u32) {
    let (a, b) = if x2 < x1 { (x2, x1) } else { (x1, x2) };
    out.push(Draw::Fill { x0: a, y0: y, x1: b + 1, y1: y + 1, color });
}
fn v_line(out: &mut Vec<Draw>, x: i32, y1: i32, y2: i32, color: u32) {
    let (a, b) = if y2 < y1 { (y2, y1) } else { (y1, y2) };
    out.push(Draw::Fill { x0: x, y0: a + 1, x1: x + 1, y1: b, color });
}
/// `RenderUtil.drawRect`.
pub fn draw_rect(x: f64, y: f64, x2: f64, y2: f64, width: f64, color: u32) -> Draw {
    let (x, y, x2, y2) = if y > y2 { (x2, y2, x, y) } else { (x, y, x2, y2) };
    let x_high = x < x2;
    let pick = |a: f64, b: f64| if x_high { a } else { b };
    Draw::Quad {
        corners: [
            [x, pick(y + width, y)],
            [x2, pick(y2 + width, y2)],
            [x2 + width, pick(y2, y2 + width)],
            [x + width, pick(y, y + width)],
        ],
        color,
    }
}
/// `drawConnection` of `child` to `parent` (their positions) with the tab's scroll: the border pass
/// (`inside`) or the coloured one. `screen_scale` is the GUI scale factor, `done` the child's progress.
pub fn connection(
    child: (i32, i32),
    parent: (i32, i32),
    scroll: (i32, i32),
    inside: bool,
    look: &Look,
    done: bool,
    screen_scale: i32,
) -> Vec<Draw> {
    let mut out = Vec::new();
    let inner = if done { look.completed_line } else { look.uncompleted_line };
    let (sx, sy) = scroll;
    if look.direct_lines {
        let half = ADVANCEMENT_SIZE / 2;
        let x1 = f64::from(sx + child.0 + half + 3);
        let y1 = f64::from(sy + child.1 + half);
        let x2 = f64::from(sx + parent.0 + half + 3);
        let y2 = f64::from(sy + parent.1 + half);
        let perpendicular = x1 == x2 || y1 == y2;
        if !perpendicular {
            let width = match screen_scale {
                1 => if inside { 1.5 } else { 0.5 },
                2 => if inside { 2.25 } else { 0.75 },
                3 => if inside { 2.0 } else { 0.666_666_666_666_666_7 },
                4 => if inside { 2.125 } else { 0.625 },
                _ => if inside { 3.0 } else { 1.0 },
            };
            if inside {
                out.push(draw_rect(x1 - 0.75, y1 - 0.75, x2 - 0.75, y2 - 0.75, width, BORDER));
            } else {
                out.push(draw_rect(x1, y1, x2, y2, width, inner));
            }
        } else {
            let width = if inside { 3.0 } else { 1.0 };
            if inside {
                out.push(draw_rect(x1 - 1.0, y1 - 1.0, x2 - 1.0, y2 - 1.0, width, BORDER));
            } else {
                out.push(draw_rect(x1, y1, x2, y2, width, inner));
            }
        }
        return out;
    }
    let start_x = sx + parent.0 + ADVANCEMENT_SIZE / 2;
    let end_x_half = sx + parent.0 + ADVANCEMENT_SIZE + 6;
    let start_y = sy + parent.1 + ADVANCEMENT_SIZE / 2;
    let end_x = sx + child.0 + ADVANCEMENT_SIZE / 2;
    let end_y = sy + child.1 + ADVANCEMENT_SIZE / 2;
    if inside {
        h_line(&mut out, end_x_half, start_x, start_y - 1, BORDER);
        h_line(&mut out, end_x_half + 1, start_x, start_y, BORDER);
        h_line(&mut out, end_x_half, start_x, start_y + 1, BORDER);
        h_line(&mut out, end_x, end_x_half - 1, end_y - 1, BORDER);
        h_line(&mut out, end_x, end_x_half - 1, end_y, BORDER);
        h_line(&mut out, end_x, end_x_half - 1, end_y + 1, BORDER);
        v_line(&mut out, end_x_half - 1, end_y, start_y, BORDER);
        v_line(&mut out, end_x_half + 1, end_y, start_y, BORDER);
    } else {
        h_line(&mut out, end_x_half, start_x, start_y, inner);
        h_line(&mut out, end_x, end_x_half, end_y, inner);
        v_line(&mut out, end_x_half, end_y, start_y, inner);
    }
    out
}

/// `isMouseOver` of a widget (inclusive on all sides), the mouse relative to the inside box.
pub fn widget_over(pos: (i32, i32), scroll: (i32, i32), mx: i32, my: i32) -> bool {
    let left = scroll.0 + pos.0;
    let top = scroll.1 + pos.1;
    mx >= left && mx <= left + ADVANCEMENT_SIZE && my >= top && my <= top + ADVANCEMENT_SIZE
}
/// `drawContents`' background tiles: the tile origin and the grid of tiles
/// (`k` from -1 to `1 + width/16`, `l` from -1 to `height/16`, then a partial row of `height % 16`).
/// Returns (first_offset_x, first_offset_y, columns of tiles, full rows, partial row height).
pub fn background_tiles(scroll: (i32, i32), width: i32, height: i32) -> (i32, i32, std::ops::RangeInclusive<i32>, std::ops::RangeInclusive<i32>, i32) {
    // Java's `%` keeps the sign of the dividend.
    (scroll.0 % 16, scroll.1 % 16, -1..=(1 + width / 16), -1..=(height / 16), height % 16)
}

// ---------------------------------------------------------------------------------------------------
// The hover box (`BetterAdvancementWidget.refreshHover` and `drawHover`).

/// `findOptimalLines`: `split(width)` gives the lines the description wraps to at that width. `empty` is the
/// description's string being empty.
pub fn find_optimal_lines<L>(
    empty: bool,
    mut split: impl FnMut(i32) -> Vec<L>,
    mut width: i32,
    internal_width: i32,
) -> Vec<L> {
    if empty {
        return Vec::new();
    }
    let mut list = split(width);
    if list.len() > 1 {
        width = width.max(internal_width / 4);
        list = split(width);
    }
    while list.len() > 5
        && f64::from(width) < f64::from(WIDGET_WIDTH) * 1.5
        && f64::from(width) < f64::from(internal_width) / 2.5
    {
        width += width / 4;
        list = split(width);
    }
    list
}
/// The room `refreshHover` keeps for the requirement counter of an advancement with several criteria:
/// `width("  ") + width("0") * digits * 2 + width("/")`.
pub fn counter_room(criteria: usize, w_two_spaces: i32, w_zero: i32, w_slash: i32) -> i32 {
    if criteria > 1 {
        let digits = criteria.to_string().len() as i32;
        w_two_spaces + w_zero * digits * 2 + w_slash
    } else {
        0
    }
}
/// The widest line before the description: `29 + title + counter`, or the grid when it is wider.
pub fn base_width(title_width: i32, counter: i32, grid_width: i32, show_grid: bool) -> i32 {
    let title = 29 + title_width + counter;
    if show_grid { title.max(grid_width) } else { title }
}
/// `showGrid`: `!requiresShift || Screen.hasShiftDown()`.
pub fn show_grid(requires_shift: bool, shift: bool) -> bool {
    !requires_shift || shift
}

/// Everything `drawHover` needs the widget and the screen to say.
#[derive(Clone, Copy, Debug)]
pub struct HoverIn {
    /// The widget's position and its tab's scroll.
    pub pos: (i32, i32),
    pub scroll: (i32, i32),
    /// The box's width (`this.width`).
    pub width: i32,
    pub desc_lines: i32,
    pub grid_height: i32,
    pub show_grid: bool,
    /// `AdvancementProgress.getPercent()`.
    pub percent: f32,
    /// The window's left and top, the screen's internal width and its real height.
    pub window_left: i32,
    pub window_top: i32,
    pub internal_width: i32,
    pub screen_height: i32,
    /// The width of the progress text, 0 when there is none.
    pub progress_width: i32,
}
/// The shape of the hover box and where its parts go (relative to the inside box's corner).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverPlan {
    pub draw_left: bool,
    pub draw_top: bool,
    /// The left piece of the title bar is `j` wide.
    pub j: i32,
    pub title_left_obtained: bool,
    pub title_right_obtained: bool,
    pub icon_obtained: bool,
    pub draw_x: i32,
    pub draw_y: i32,
    pub box_height: i32,
    /// The description box's y (`drawTop ? drawY + 26 - boxHeight : drawY`).
    pub box_y: i32,
    pub title_x: i32,
    pub title_y: i32,
    pub progress_x: i32,
    pub desc_x: i32,
    pub desc_y: i32,
    pub grid_x: i32,
    pub grid_y: i32,
}
/// `drawHover`'s arithmetic.
pub fn plan_hover(i: &HoverIn) -> HoverPlan {
    let (x, y) = i.pos;
    let (sx, sy) = i.scroll;
    let draw_left = i.window_left + sx + x + i.width + ADVANCEMENT_SIZE >= i.internal_width;
    let draw_top = if i.show_grid {
        if i.grid_height < i.screen_height {
            i.window_top + sy + y + i.desc_lines * LINE_HEIGHT + i.grid_height + 50 >= i.screen_height
        } else {
            false
        }
    } else {
        i.window_top + sy + y + i.desc_lines * LINE_HEIGHT + 50 >= i.screen_height
    };
    let mut j = (i.percent * i.width as f32).floor() as i32;
    let (left, right, icon);
    if i.percent >= 1.0 {
        j = i.width / 2;
        (left, right, icon) = (true, true, true);
    } else if j < 2 {
        j = i.width / 2;
        (left, right, icon) = (false, false, false);
    } else if j > i.width - 2 {
        j = i.width / 2;
        (left, right, icon) = (true, true, false);
    } else {
        (left, right, icon) = (true, false, false);
    }
    let draw_y = sy + y;
    let draw_x = if draw_left { sx + x - i.width + ADVANCEMENT_SIZE + 6 } else { sx + x };
    let box_height = TITLE_SIZE + i.desc_lines * LINE_HEIGHT + if i.show_grid { i.grid_height } else { 0 };
    let box_y = if draw_top { draw_y + ADVANCEMENT_SIZE - box_height } else { draw_y };
    let (title_x, progress_x) = if draw_left {
        (draw_x + 5, sx + x - i.progress_width)
    } else {
        (sx + x + 32, sx + x + i.width - i.progress_width - 5)
    };
    let desc_y = if draw_top { draw_y + 26 - box_height + 7 } else { sy + y + 9 + 17 };
    HoverPlan {
        draw_left,
        draw_top,
        j,
        title_left_obtained: left,
        title_right_obtained: right,
        icon_obtained: icon,
        draw_x,
        draw_y,
        box_height,
        box_y,
        title_x,
        title_y: sy + y + 9,
        progress_x,
        desc_x: draw_x + 5,
        desc_y,
        grid_x: draw_x + 5,
        grid_y: desc_y + i.desc_lines * LINE_HEIGHT,
    }
}
/// `AdvancementProgress.getPercent`.
pub fn percent(done_requirements: usize, requirements: usize) -> f32 {
    if requirements == 0 { 0.0 } else { done_requirements as f32 / requirements as f32 }
}
/// `AdvancementProgress.getProgressText`: `done/total`, none for one requirement or fewer.
pub fn progress_text(done_requirements: usize, requirements: usize) -> Option<String> {
    (requirements > 1).then(|| format!("{done_requirements}/{requirements}"))
}
/// `TabType.fade` of a tab, each time its tooltips are drawn.
pub fn fade_step(fade: f32, hovering: bool, do_fade: bool) -> f32 {
    if do_fade && hovering {
        clamp_f32(fade + 0.02, 0.0, 0.3)
    } else {
        clamp_f32(fade - 0.04, 0.0, 1.0)
    }
}
/// `Mth.floor(fade * 255.0F)`: the alpha of the dark veil over the tab.
pub fn fade_alpha(fade: f32) -> u32 {
    (fade * 255.0).floor() as u32
}

// ---------------------------------------------------------------------------------------------------
// The criteria list (`CriterionGrid`, `CriterionTranslator`).

pub const GREEN: u32 = 0x55FF55;
pub const DARK_RED: u32 = 0xAA0000;
pub const TEXT_WHITE: u32 = 0xFFFFFF;

/// What a cell shows after its marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// A criterion's name, to be translated (`CriterionTranslator`).
    Criterion(String),
    /// `betteradvancements.remaining` with the count.
    Remaining(usize),
}
/// A cell: ` + ` or ` x ` in a colour, then the body in another (italic for the remaining count).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub marker: &'static str,
    pub marker_color: u32,
    pub body: Body,
    pub body_color: u32,
    pub italic: bool,
}
/// The cells `findOptimalCriterionGrid` makes of an advancement's criteria. `criteria` are the names with
/// whether each is obtained, in the order of the client's map (`java_map_order`). None when no list is shown:
/// off, done, or a single criterion.
pub fn build_cells(detail: Detail, color_whole: bool, done: bool, criteria: &[(String, bool)]) -> Option<Vec<Cell>> {
    if done || detail == Detail::Off || criteria.len() <= 1 {
        return None;
    }
    let mut cells = Vec::new();
    let mut unobtained = 0;
    for (name, obtained) in criteria {
        if *obtained {
            if detail.show_obtained() {
                cells.push(Cell {
                    marker: " + ",
                    marker_color: GREEN,
                    body: Body::Criterion(name.clone()),
                    body_color: if color_whole { GREEN } else { TEXT_WHITE },
                    italic: false,
                });
            }
        } else {
            if detail.show_unobtained() {
                cells.push(Cell {
                    marker: " x ",
                    marker_color: DARK_RED,
                    body: Body::Criterion(name.clone()),
                    body_color: if color_whole { DARK_RED } else { TEXT_WHITE },
                    italic: false,
                });
            }
            unobtained += 1;
        }
    }
    if !detail.show_unobtained() {
        cells.push(Cell {
            marker: " x ",
            marker_color: DARK_RED,
            body: Body::Remaining(unobtained),
            body_color: if color_whole { DARK_RED } else { TEXT_WHITE },
            italic: true,
        });
    }
    Some(cells)
}

/// A column of the grid: the cells' indexes and the column's width.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub cells: Vec<usize>,
    pub width: i32,
}
/// `CriterionGrid`: the cells in columns and pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub columns: usize,
    pub pages: usize,
    pub rows: usize,
    pub width: i32,
    pub height: i32,
    pub paged: Vec<Vec<Column>>,
}
impl Grid {
    /// `CriterionGrid.empty`.
    pub fn empty() -> Grid {
        Grid { columns: 0, pages: 1, rows: 0, width: 0, height: 0, paged: Vec::new() }
    }
    /// The constructor: the shape, not yet laid out (`init`).
    fn shaped(cells: usize, columns: usize, pages: usize) -> Grid {
        let rows = (cells as f64 / columns as f64 / pages as f64).ceil() as usize;
        Grid { columns, pages, rows, width: 0, height: 0, paged: Vec::new() }
    }
    /// `init`.
    fn lay_out(&mut self, widths: &[i32], font_height: i32) {
        self.paged = Vec::new();
        self.width = 0;
        for p in 0..self.pages {
            let mut columns = Vec::new();
            let mut page_width = 0;
            for c in 0..self.columns {
                let mut cells = Vec::new();
                let mut column_width = 0;
                for r in 0..self.rows {
                    let index = p * self.columns * self.rows + c * self.rows + r;
                    if index >= widths.len() {
                        break;
                    }
                    cells.push(index);
                    column_width = column_width.max(widths[index]);
                }
                columns.push(Column { cells, width: column_width });
                page_width += column_width;
            }
            self.paged.push(columns);
            self.width = self.width.max(page_width);
        }
        self.height = self.rows as i32 * font_height;
    }
    /// `hasPages`.
    pub fn has_pages(&self) -> bool {
        !self.paged.is_empty()
    }
    /// `getCurrentPage`: `currentTimeMillis / (speed * 1000) % pages`.
    pub fn current_page(&self, now_millis: i64, rotation_speed: i32) -> usize {
        let step = i64::from(rotation_speed.max(1)) * 1000;
        ((now_millis / step) % self.pages.max(1) as i64) as usize
    }
    /// `findOptimalCriterionGrid` after the cells: the loops are the original's, `continue` of its do-while
    /// included (it jumps to the condition with the grids as they were). Where Java would crash on a null grid
    /// (a first column already wider than `max_width`) the single-column grid is used.
    pub fn find(widths: &[i32], font_height: i32, max_width: i32, max_height: i32) -> Grid {
        let n = widths.len();
        let mut columns = 0usize;
        let mut pages = 1usize;
        let mut prev: Option<Grid> = None;
        let mut curr: Option<Grid> = None;
        let mut first: Option<Grid> = None;
        loop {
            columns += 1;
            let mut grid = Grid::shaped(n, columns, pages);
            let skip = prev.as_ref().is_some_and(|p| grid.rows == p.rows);
            if !skip {
                grid.lay_out(widths, font_height);
                if first.is_none() {
                    first = Some(grid.clone());
                }
                prev = curr.take();
                curr = Some(grid);
            }
            let wide_enough = curr.as_ref().is_some_and(|g| g.width <= max_width);
            if !(columns <= n && wide_enough) {
                break;
            }
        }
        columns -= 1;
        let mut curr = match prev.take() {
            Some(g) => g,
            None => {
                columns = 1;
                first.unwrap_or_else(Grid::empty)
            }
        };
        let columns = columns.max(1);
        let mut prev: Option<Grid> = None;
        while pages <= n && curr.height > max_height {
            pages += 1;
            let mut grid = Grid::shaped(n, columns, pages);
            if prev.as_ref().is_some_and(|p| grid.rows == p.rows) {
                continue;
            }
            grid.lay_out(widths, font_height);
            prev = Some(std::mem::replace(&mut curr, grid));
        }
        curr
    }
}

/// `CriterionTranslator`'s candidates, in order, and the plain key and fallback of the last resort.
pub fn translation_keys(advancement: &str, criterion: &str) -> (Vec<String>, String) {
    let mut prepped = criterion.replace(':', ".");
    if !criterion.contains(':') {
        let namespace = advancement.split_once(':').map_or("minecraft", |(n, _)| n);
        prepped = format!("{namespace}.{prepped}");
    }
    let mut keys = vec![
        prepped.clone(),
        format!("betteradvancements.criterion.{advancement}.{criterion}"),
        format!("biome.{prepped}"),
        format!("item.{prepped}"),
        format!("block.{prepped}"),
        format!("entity.{prepped}"),
    ];
    if criterion.starts_with("armor_trimmed") {
        let after = java_split(criterion, ':').get(1).cloned();
        let before = java_split(criterion, ':').first().cloned().unwrap_or_default();
        let origin = java_split(&before.replace("armor_trimmed", ""), '_').get(1).cloned();
        if let (Some(after), Some(origin)) = (after, origin)
            && let Some(kind) = java_split(&after, '_').first().cloned()
        {
            keys.push(format!("trim_pattern.{origin}.{kind}"));
            keys.push(format!("trim_material.{origin}.{kind}"));
        }
    }
    (keys, prepped)
}

// ---------------------------------------------------------------------------------------------------
// Smaller things.

/// `AdvancementComparer.sortByTitle` on two titles (`toLowerCase`, then `String.compareTo`, UTF-16 order).
pub fn compare_titles(a: &str, b: &str) -> Ordering {
    let a: Vec<u16> = a.to_lowercase().encode_utf16().collect();
    let b: Vec<u16> = b.to_lowercase().encode_utf16().collect();
    a.cmp(&b)
}
/// The debug coordinates text (`showDebugCoordinates`): the text and where it goes, when the mouse is over the
/// inside (by the original's own, odd, bounds).
pub fn debug_coordinates(w: &Window, mouse: (i32, i32), scroll: (i32, i32)) -> Option<(String, i32, i32)> {
    let (mx, my) = mouse;
    if mx < w.internal_width - SIDE - PADDING
        && mx > SIDE + PADDING
        && my < w.internal_height - w.top + 1
        && my > w.top + PADDING * 2
    {
        let x_mouse = mx - w.left - PADDING;
        let y_mouse = my - w.top - 2 * PADDING;
        let x = x_mouse - scroll.0 - 3 - 1;
        let y = y_mouse - scroll.1 - 1;
        Some((format!("{x},{y}"), mx, my - LINE_HEIGHT))
    } else {
        None
    }
}

}

pub const ID: &str = "betteradvancements";
pub const TITLE: &str = "Better Advancements 0.6.0.73";
pub const DESCRIPTION: &str = "A larger advancements window that follows the screen size, tabs that page, hover boxes with the list of criteria, and the original's colours and options.";

/// The layout of the loaded tree: widget pixel positions (`floor(x * 32)`, `floor(y * 27)`), and who is whose child.
struct Cache {
    tree: Arc<Tree>,
    server: bool,
    kids: Vec<Vec<usize>>,
    pos: Vec<Option<(i32, i32)>>,
}
struct State {
    config: Option<engine::Config>,
    cache: Option<Cache>,
    paging: engine::Paging,
    tabs: engine::Tabs,
    history: engine::History,
    drag: Option<(f32, f32)>,
    fade: f32,
    last_frame: Option<std::time::Instant>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        config: None,
        cache: None,
        paging: engine::Paging::default(),
        tabs: engine::Tabs::default(),
        history: engine::History::default(),
        drag: None,
        fade: 0.0,
        last_frame: None,
    });
}

/// What one frame shows.
struct View {
    /// The tabs' roots (indexes in the tree), in the order they are listed.
    tabs: Vec<usize>,
    /// The selected root, if any.
    selected: Option<usize>,
    /// Its visible widgets in tree order, and each one's nearest displayed ancestor.
    widgets: Vec<usize>,
    parent: Vec<Option<usize>>,
    bounds: engine::Bounds,
}

fn rgba_of(argb: u32) -> u32 {
    rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}
fn rgb_of(rgb: u32) -> u32 {
    rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
}
fn frame_sprite(frame: Frame, obtained: bool) -> &'static str {
    match (frame, obtained) {
        (Frame::Task, true) => "gui/sprites/advancements/task_frame_obtained",
        (Frame::Task, false) => "gui/sprites/advancements/task_frame_unobtained",
        (Frame::Goal, true) => "gui/sprites/advancements/goal_frame_obtained",
        (Frame::Goal, false) => "gui/sprites/advancements/goal_frame_unobtained",
        (Frame::Challenge, true) => "gui/sprites/advancements/challenge_frame_obtained",
        (Frame::Challenge, false) => "gui/sprites/advancements/challenge_frame_unobtained",
    }
}
fn box_sprite(obtained: bool) -> &'static str {
    if obtained {
        "gui/sprites/advancements/box_obtained"
    } else {
        "gui/sprites/advancements/box_unobtained"
    }
}
fn tab_sprite(side: engine::Side, column: i32, selected: bool) -> String {
    let (first, middle, last) = match side {
        engine::Side::Above => ("above_left", "above_middle", "above_right"),
        engine::Side::Left => ("left_top", "left_middle", "left_bottom"),
        engine::Side::Right => ("right_top", "right_middle", "right_bottom"),
    };
    let part = match column {
        0 => first,
        1 => middle,
        _ => last,
    };
    format!("gui/sprites/advancements/tab_{part}{}", if selected { "_selected" } else { "" })
}

impl Game<'_> {
    fn betteradvancements_config(&self) -> engine::Config {
        STATE.with(|s| {
            s.borrow_mut()
                .config
                .get_or_insert_with(|| engine::Config::read(|key| self.native_setting(ID, key)))
                .clone()
        })
    }

    /// The tree's layout, cached per loaded tree.
    fn betteradvancements_cache(&self) {
        let tree = self.adv.tree.clone();
        let server = self.adv.positions.is_some();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.cache.as_ref().is_some_and(|c| Arc::ptr_eq(&c.tree, &tree) && c.server == server) {
                return;
            }
            let kids = tree.children();
            let pos = match &self.adv.positions {
                Some(placed) => placed.iter().map(|&(x, y)| Some(engine::widget_pos(x, y))).collect(),
                None => tree
                    .layout()
                    .into_iter()
                    .map(|p| p.map(|(x, y)| engine::widget_pos(x as f32, y)))
                    .collect(),
            };
            s.cache = Some(Cache { tree, server, kids, pos });
        });
    }

    fn betteradvancements_view(&self, cfg: &engine::Config) -> View {
        self.betteradvancements_cache();
        STATE.with(|s| {
            let s = s.borrow();
            let c = s.cache.as_ref().expect("cache");
            let tree = &c.tree;
            let visible = if self.adv.positions.is_some() {
                vec![true; tree.list.len()]
            } else {
                tree.visible(&c.kids, |i| self.adv.progress.done(&tree.list[i]))
            };
            // `onAddAdvancementRoot`: a displayed root with a background.
            let mut tabs: Vec<usize> = (0..tree.list.len())
                .filter(|&i| {
                    let a = &tree.list[i];
                    a.parent.is_none() && visible[i] && a.display.as_ref().is_some_and(|d| d.background.is_some())
                })
                .collect();
            const ORDER: [&str; 5] = [
                "minecraft:story/root",
                "minecraft:nether/root",
                "minecraft:end/root",
                "minecraft:adventure/root",
                "minecraft:husbandry/root",
            ];
            tabs.sort_by_key(|&i| {
                let id = &tree.list[i].id;
                (ORDER.iter().position(|r| r == id).unwrap_or(ORDER.len()), id.clone())
            });
            if cfg.order_tabs_alphabetically {
                let title = |i: usize| {
                    tree.list[i]
                        .display
                        .as_ref()
                        .map(|d| super::advancements::text_of(&d.title).plain())
                        .unwrap_or_default()
                };
                tabs.sort_by(|&a, &b| engine::compare_titles(&title(a), &title(b)));
            }
            let selected = s
                .tabs
                .selected
                .as_ref()
                .and_then(|id| tabs.iter().copied().find(|&i| tree.list[i].id == *id))
                .or_else(|| tabs.first().copied());
            let mut parent = vec![None; tree.list.len()];
            let mut widgets = Vec::new();
            let mut positions = Vec::new();
            if let Some(t) = selected {
                let mut stack = vec![(t, None::<usize>)];
                while let Some((i, up)) = stack.pop() {
                    let shown = visible[i] && tree.list[i].display.is_some() && c.pos[i].is_some();
                    let next = if shown {
                        parent[i] = up;
                        widgets.push(i);
                        positions.push(c.pos[i].unwrap_or_default());
                        Some(i)
                    } else {
                        up
                    };
                    for &k in c.kids[i].iter().rev() {
                        stack.push((k, next));
                    }
                }
            }
            View { tabs, selected, widgets, parent, bounds: engine::Bounds::of(&positions) }
        })
    }

    /// `AdvancementProgress` of the player, per criterion of `a` (names in the client's map order).
    fn betteradvancements_criteria(&self, a: &Advancement) -> Vec<(String, bool)> {
        let names: Vec<String> = a.criteria.iter().map(|c| c.name.clone()).collect();
        engine::java_map_order(&names)
            .into_iter()
            .map(|i| (names[i].clone(), self.adv.progress.has(&a.id, &names[i])))
            .collect()
    }

    /// `CriterionTranslator`: the first key the language has, else the plain name.
    fn betteradvancements_translate(&self, advancement: &str, criterion: &str) -> String {
        let (keys, plain) = engine::translation_keys(advancement, criterion);
        keys.iter()
            .find(|k| horizon_client::lang::has(k))
            .map(|k| horizon_client::lang::get(k))
            .unwrap_or(plain)
    }

    fn betteradvancements_cell_text(&self, adv: &str, cell: &engine::Cell) -> (Text, Text) {
        let body = match &cell.body {
            engine::Body::Criterion(name) => Text::literal(self.betteradvancements_translate(adv, name)),
            engine::Body::Remaining(n) => {
                Text::translate("betteradvancements.remaining", &[Text::literal(n.to_string())])
            }
        };
        let body = if cell.italic { body.italic() } else { body };
        (Text::literal(cell.marker), body)
    }

    pub(super) fn betteradvancements_open(&mut self) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tabs.scrolling = false;
            s.drag = None;
            s.fade = 0.0;
        });
    }

    /// `BetterAdvancementsScreen.render`.
    pub(super) fn betteradvancements_draw(&mut self, batch: &mut HudBatch) {
        let cfg = self.betteradvancements_config();
        let look = engine::Look::from_config(&cfg);
        let view = self.betteradvancements_view(&cfg);
        let (sw, sh) = (batch.width as i32, batch.height as i32);
        let w = engine::Window::new(sw, sh, cfg.ui_scaling);
        let (width, height) = w.render_size();
        let per_page = engine::max_tabs(width, height, cfg.only_use_above_tabs);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.paging.init(view.tabs.len() as i32, per_page);
        });
        // The selected tab, and its scroll.
        let tree = self.adv.tree.clone();
        let root_id = view.selected.map(|i| tree.list[i].id.clone());
        if let Some(root) = &root_id {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                if s.tabs.selected.as_ref() != Some(root) {
                    let State { tabs, history, .. } = &mut *s;
                    tabs.select(root, &view.bounds, history, &w);
                }
            });
        }
        let scroll = match &root_id {
            Some(root) => STATE.with(|s| s.borrow_mut().tabs.frame_scroll(root, &view.bounds, &w)),
            None => (0, 0),
        };
        let (bl, bt, br, bb) = w.box_rect();
        let (bw, bh) = (br - bl, bb - bt);
        let f = |v: i32| v as f32;
        let (paging, tab_page) = STATE.with(|s| {
            let s = s.borrow();
            (s.paging, s.paging.range(per_page, view.tabs.len()))
        });

        {
            let atlas = &self.gui.atlas;
            ui::menu_background(batch, atlas, 0.0, 0.0, f(sw), f(sh), true);
            if paging.max_pages != 0 {
                let label = Text::literal(format!("{} / {}", paging.page + 1, paging.max_pages + 1));
                let tw = atlas.font.width_text(&label) as i32;
                let (_, x, y) = paging.label(tw, &w);
                batch.text_styled(atlas, &label, f(x), f(y), u32::MAX, true);
                for (n, (x, y, bw, bh)) in engine::page_buttons(&w).into_iter().enumerate() {
                    let hover = ui::hovered(self.cursor, f(x), f(y), f(bw), f(bh));
                    let label = Text::literal(if n == 0 { "<" } else { ">" });
                    let active = if n == 0 { paging.page > 0 } else { paging.page < paging.max_pages };
                    ui::button(batch, atlas, f(x), f(y), f(bw), f(bh), &label, active, hover);
                }
            }
        }

        // The inside (`renderInside` / `drawContents`).
        match view.selected {
            None => {
                let atlas = &self.gui.atlas;
                batch.fill(atlas, f(bl), f(bt), f(bw), f(bh), 0xFF00_0000);
                for (key, y) in [
                    ("advancements.empty", bt + bh / 2 - 9),
                    ("advancements.sad_label", bt + bh / 2 + 9),
                ] {
                    let t = Text::translate(key, &[]);
                    let tw = atlas.font.width_text(&t) as i32;
                    batch.text_styled(atlas, &t, f(bl + (bw - tw) / 2), f(y), u32::MAX, true);
                }
            }
            Some(root) => {
                let start = batch.vertices.len();
                let (origin_x, origin_y) = (bl, bt);
                let bg = tree.list[root].display.as_ref().and_then(|d| d.background.clone()).unwrap_or_default();
                let bg = bg.strip_prefix("minecraft:").unwrap_or(&bg).to_owned();
                let (i0, j0, ks, ls, rest) = engine::background_tiles(scroll, bw, bh);
                for k in ks {
                    let mut last_l = -1;
                    for l in ls.clone() {
                        batch.sprite_scaled(
                            &self.gui.atlas,
                            &bg,
                            f(origin_x + i0 + 16 * k),
                            f(origin_y + j0 + 16 * l),
                            16.0,
                            16.0,
                            u32::MAX,
                        );
                        last_l = l;
                    }
                    // The partial row of `height % 16`: its texels are the top ones.
                    if rest > 0 {
                        let y = origin_y + j0 + 16 * (last_l + 1);
                        batch.sprite_region(
                            &self.gui.atlas,
                            &bg,
                            [f(origin_x + i0 + 16 * k), f(y), 16.0, f(rest)],
                            16.0,
                            16.0,
                        );
                    }
                }
                let pos_of = |i: usize| {
                    STATE.with(|s| s.borrow().cache.as_ref().and_then(|c| c.pos[i]).unwrap_or_default())
                };
                let scale = batch.scale as i32;
                if !look.hide_lines {
                    for inside in [true, false] {
                        for &i in &view.widgets {
                            let Some(p) = view.parent[i] else { continue };
                            let done = self.adv.progress.done(&tree.list[i]);
                            let draws = engine::connection(pos_of(i), pos_of(p), scroll, inside, &look, done, scale);
                            self.betteradvancements_draws(batch, &draws, (origin_x, origin_y));
                        }
                    }
                }
                for &i in &view.widgets {
                    let a = &tree.list[i];
                    let Some(d) = &a.display else { continue };
                    let done = self.adv.progress.done(a);
                    if d.hidden && !done {
                        continue;
                    }
                    let (x, y) = pos_of(i);
                    let (x, y) = (f(origin_x + scroll.0 + x), f(origin_y + scroll.1 + y));
                    let tint = if look.has_custom_icon_color() {
                        rgba_of(look.icon_color(done))
                    } else {
                        u32::MAX
                    };
                    batch.sprite_scaled(&self.gui.atlas, frame_sprite(d.frame, done), x + 3.0, y, 26.0, 26.0, tint);
                    self.draw_item(batch, &ItemStack::new(&d.icon, 1), x + 8.0, y + 5.0);
                }
                batch.clip_since(start, f(bl), f(bt), f(br), f(bb));
            }
        }

        // `renderWindow`: the frame (`window.png`: corners of 30, the sides repeat), the tabs and the title.
        self.betteradvancements_window(batch, &w);
        let atlas = &self.gui.atlas;
        if view.tabs.len() > 1 {
            for (n, ti) in tab_page.clone().enumerate() {
                let global = ti as i32;
                let _ = n;
                let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
                let x = w.left + engine::tab_x(side, global, width, height);
                let y = w.top + engine::tab_y(side, global, width, height);
                let (tw, th) = side.size();
                let column = engine::tab_column(side, w.left, width, height, global);
                let selected = Some(view.tabs[ti]) == view.selected;
                batch.sprite_scaled(atlas, &tab_sprite(side, column, selected), f(x), f(y), f(tw), f(th), u32::MAX);
            }
            for ti in tab_page.clone() {
                let global = ti as i32;
                let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
                let x = w.left + engine::tab_x(side, global, width, height);
                let y = w.top + engine::tab_y(side, global, width, height);
                let (dx, dy) = engine::tab_icon_offset(side);
                if let Some(d) = &tree.list[view.tabs[ti]].display {
                    self.draw_item(batch, &ItemStack::new(&d.icon, 1), f(x + dx), f(y + dy));
                }
            }
        }
        let atlas = &self.gui.atlas;
        let title = Text::translate("gui.advancements", &[]);
        let title = match view.selected.and_then(|t| tree.list[t].display.as_ref()) {
            Some(d) => {
                let mut full = title;
                full = Text::literal(format!("{} - {}", full.plain(), super::advancements::text_of(&d.title).plain()));
                full
            }
            None => title,
        };
        batch.text_styled(atlas, &title, f(w.left + 8), f(w.top + 6), 0xFF40_4040, false);

        // `renderToolTips`: the veil and the hover box of the widget under the cursor.
        if let Some(root) = view.selected {
            let (ox, oy) = (w.left + engine::PADDING, w.top + 2 * engine::PADDING);
            let (tw, th) = w.tooltip_size();
            let (mx, my) = (self.cursor.0.floor() as i32 - ox, self.cursor.1.floor() as i32 - oy);
            let mut hovered = None;
            if mx > 0 && mx < tw && my > 0 && my < th {
                for &i in &view.widgets {
                    let a = &tree.list[i];
                    if a.display.as_ref().is_some_and(|d| d.hidden) && !self.adv.progress.done(a) {
                        continue;
                    }
                    let pos = STATE.with(|s| s.borrow().cache.as_ref().and_then(|c| c.pos[i]).unwrap_or_default());
                    if engine::widget_over(pos, scroll, mx, my) {
                        hovered = Some((i, pos));
                        break;
                    }
                }
            }
            let _ = root;
            let fade = STATE.with(|s| {
                let mut s = s.borrow_mut();
                let (fade, now) = (s.fade, std::time::Instant::now());
                // The original steps once per drawn frame at 20 ticks a second's pace of `render`; frames here
                // are scaled to the tick so the fade lasts the same.
                let ticks = s.last_frame.map_or(1.0, |t| (now - t).as_secs_f32() * 20.0).min(5.0);
                s.last_frame = Some(now);
                let mut f = fade;
                let steps = ticks.max(0.0);
                let whole = steps.floor() as i32;
                for _ in 0..whole.max(1) {
                    f = engine::fade_step(f, hovered.is_some(), cfg.do_fade);
                }
                s.fade = f;
                f
            });
            let atlas = &self.gui.atlas;
            batch.fill(atlas, f(ox), f(oy), f(tw), f(th), engine::fade_alpha(fade) << 24);
            if let Some((i, pos)) = hovered {
                self.betteradvancements_hover(batch, &cfg, &look, &w, i, pos, scroll, (ox, oy));
            }
            if cfg.show_debug_coordinates {
                if let Some((text, x, y)) =
                    engine::debug_coordinates(&w, (self.cursor.0 as i32, self.cursor.1 as i32), scroll)
                {
                    batch.text_styled(&self.gui.atlas, &Text::literal(text), f(x), f(y), 0xFF00_0000, false);
                }
            }
        }
        // The tab titles.
        if view.tabs.len() > 1 {
            for ti in tab_page {
                let global = ti as i32;
                let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
                if engine::tab_over(side, w.left, w.top, width, height, global, f64::from(self.cursor.0), f64::from(self.cursor.1)) {
                    if let Some(d) = &tree.list[view.tabs[ti]].display {
                        ui::tooltip(batch, &self.gui.atlas, &[super::advancements::text_of(&d.title)], self.cursor);
                    }
                }
            }
        }
    }

    /// The window frame of `renderWindow`: `window.png` is 252 x 140; the corners are 30 and the sides repeat.
    fn betteradvancements_window(&self, batch: &mut HudBatch, w: &engine::Window) {
        let atlas = &self.gui.atlas;
        let name = "gui/advancements/window";
        let Some(r) = atlas.sprites.get(name).copied() else { return };
        // The sheet is 256 x 256 texels in the atlas with the window in its top left 252 x 140.
        let (tw, th) = (256.0f32, 256.0f32);
        let piece = |batch: &mut HudBatch, x: i32, y: i32, dw: i32, dh: i32, u: i32, v: i32, sw: i32, sh: i32| {
            if dw <= 0 || dh <= 0 || sw <= 0 || sh <= 0 {
                return;
            }
            let rect = horizon_client::hud::Rect {
                u0: r.u0 + (r.u1 - r.u0) * u as f32 / tw,
                v0: r.v0 + (r.v1 - r.v0) * v as f32 / th,
                u1: r.u0 + (r.u1 - r.u0) * (u + sw) as f32 / tw,
                v1: r.v0 + (r.v1 - r.v0) * (v + sh) as f32 / th,
                w: sw as u32,
                h: sh as u32,
            };
            batch.rect_uv(x as f32, y as f32, dw as f32, dh as f32, rect, u32::MAX);
        };
        // `RenderUtil.renderRepeating`: the source piece repeats to fill the destination, the last tile cut.
        let repeat = |batch: &mut HudBatch, x: i32, y: i32, dw: i32, dh: i32, u: i32, v: i32, sw: i32, sh: i32| {
            if sw <= 0 || sh <= 0 {
                return;
            }
            let mut oy = 0;
            while oy < dh {
                let h = sh.min(dh - oy);
                let mut ox = 0;
                while ox < dw {
                    let wd = sw.min(dw - ox);
                    piece(batch, x + ox, y + oy, wd, h, u, v, wd, h);
                    ox += sw;
                }
                oy += sh;
            }
        };
        let (cs, ww, wh) = (engine::CORNER, engine::WINDOW_TEXTURE_W, engine::WINDOW_TEXTURE_H);
        let (left, top, right, bottom) = (w.left, w.top, w.right, w.bottom);
        let across = w.internal_width - cs - 2 * engine::SIDE - cs;
        piece(batch, left, top, cs, cs, 0, 0, cs, cs);
        repeat(batch, left + cs, top, across, cs, cs, 0, ww - 2 * cs, cs);
        piece(batch, right - cs, top, cs, cs, ww - cs, 0, cs, cs);
        repeat(batch, left, top + cs, cs, bottom - top - 2 * cs, 0, cs, cs, wh - 2 * cs);
        repeat(batch, right - cs, top + cs, cs, bottom - top - 2 * cs, ww - cs, cs, cs, wh - 2 * cs);
        piece(batch, left, bottom - cs, cs, cs, 0, wh - cs, cs, cs);
        repeat(batch, left + cs, bottom - cs, across, cs, cs, wh - cs, ww - 2 * cs, cs);
        piece(batch, right - cs, bottom - cs, cs, cs, ww - cs, wh - cs, cs, cs);
    }

    fn betteradvancements_draws(&self, batch: &mut HudBatch, draws: &[engine::Draw], origin: (i32, i32)) {
        let atlas = &self.gui.atlas;
        let (ox, oy) = (origin.0 as f32, origin.1 as f32);
        for d in draws {
            match d {
                engine::Draw::Fill { x0, y0, x1, y1, color } => {
                    let (ax, bx) = ((*x0).min(*x1), (*x0).max(*x1));
                    let (ay, by) = ((*y0).min(*y1), (*y0).max(*y1));
                    batch.fill(atlas, ox + ax as f32, oy + ay as f32, (bx - ax) as f32, (by - ay) as f32, rgba_of(*color));
                }
                engine::Draw::Quad { corners, color } => {
                    let c = corners.map(|p| [ox + p[0] as f32, oy + p[1] as f32]);
                    batch.fill_quad(atlas, c, rgba_of(*color));
                }
            }
        }
    }

    /// `BetterAdvancementWidget.drawHover`.
    #[allow(clippy::too_many_arguments)]
    fn betteradvancements_hover(
        &self,
        batch: &mut HudBatch,
        cfg: &engine::Config,
        look: &engine::Look,
        w: &engine::Window,
        i: usize,
        pos: (i32, i32),
        scroll: (i32, i32),
        origin: (i32, i32),
    ) {
        let tree = self.adv.tree.clone();
        let a = &tree.list[i];
        let Some(d) = &a.display else { return };
        let atlas = &self.gui.atlas;
        let font = &atlas.font;
        let shift = self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        let show = engine::show_grid(cfg.requires_shift, shift);
        // `refreshHover`.
        let title: String = super::advancements::text_of(&d.title).plain().chars().take(163).collect();
        let k = engine::counter_room(
            a.criteria.len(),
            font.width("  ") as i32,
            font.width("0") as i32,
            font.width("/") as i32,
        );
        let title_width = 29 + font.width(&title) as i32 + k;
        let done = self.adv.progress.done(a);
        let criteria = self.betteradvancements_criteria(a);
        let cells = engine::build_cells(cfg.detail, cfg.color_whole_criteria_text, done, &criteria);
        let cell_texts: Vec<(Text, Text)> = cells
            .as_ref()
            .map(|c| c.iter().map(|cell| self.betteradvancements_cell_text(&a.id, cell)).collect())
            .unwrap_or_default();
        let widths: Vec<i32> = cell_texts.iter().map(|(m, b)| (font.width_text(m) + font.width_text(b)) as i32).collect();
        let grid = if cells.is_some() {
            engine::Grid::find(&widths, engine::LINE_HEIGHT, w.width / 2, w.height / 2)
        } else {
            engine::Grid::empty()
        };
        let max_width = if show { title_width.max(grid.width) } else { title_width };
        let color = match d.frame {
            Frame::Challenge => 0xAA00AA,
            _ => 0x55FF55,
        };
        let desc_text = super::advancements::text_of(&d.description).with_style(&Style::color(color));
        let empty = desc_text.plain().is_empty();
        let mut desc = engine::find_optimal_lines(
            empty,
            |width| font.split(&desc_text, width as f32),
            max_width,
            w.internal_width,
        );
        let mut width = max_width;
        for l in &desc {
            width = width.max(font.width_text(l) as i32);
        }
        let width = width + 8;
        let (got, total) = self.adv.progress.counts(a);
        let percent = if a.criteria.is_empty() { 0.0 } else { engine::percent(got, total) };
        let progress = if a.criteria.is_empty() { None } else { engine::progress_text(got, total) };
        let progress_width = progress.as_ref().map_or(0, |p| font.width(p) as i32);
        let plan = engine::plan_hover(&engine::HoverIn {
            pos,
            scroll,
            width,
            desc_lines: desc.len() as i32,
            grid_height: grid.height,
            show_grid: show,
            percent,
            window_left: w.left,
            window_top: w.top,
            internal_width: w.internal_width,
            screen_height: w.height,
            progress_width,
        });
        let f = |v: i32| v as f32;
        let (ox, oy) = origin;
        let tint_title = |obtained: bool| {
            if look.has_custom_title_color() { rgba_of(look.title_color(obtained)) } else { u32::MAX }
        };
        if !desc.is_empty() {
            ui::nine_slice(
                batch,
                atlas,
                "gui/sprites/advancements/title_box",
                f(ox + plan.draw_x),
                f(oy + plan.box_y),
                f(width),
                f(plan.box_height),
                u32::MAX,
            );
        }
        let (first_w, second_w) = (plan.j, width - plan.j);
        if plan.title_left_obtained != plan.title_right_obtained {
            for (obtained, src_x, at, wd) in [
                (plan.title_left_obtained, 0, plan.draw_x, first_w),
                (plan.title_right_obtained, 200 - second_w, plan.draw_x + first_w, second_w),
            ] {
                let start = batch.vertices.len();
                ui::nine_slice(
                    batch,
                    atlas,
                    box_sprite(obtained),
                    f(ox + at - src_x),
                    f(oy + plan.draw_y),
                    200.0,
                    26.0,
                    tint_title(obtained),
                );
                batch.clip_since(start, f(ox + at), f(oy + plan.draw_y), f(ox + at + wd), f(oy + plan.draw_y + 26));
            }
        } else {
            ui::nine_slice(
                batch,
                atlas,
                box_sprite(plan.title_left_obtained),
                f(ox + plan.draw_x),
                f(oy + plan.draw_y),
                f(width),
                26.0,
                tint_title(plan.title_left_obtained),
            );
        }
        let icon_tint = if look.has_custom_icon_color() { rgba_of(look.icon_color(plan.icon_obtained)) } else { u32::MAX };
        batch.sprite_scaled(
            atlas,
            frame_sprite(d.frame, plan.icon_obtained),
            f(ox + scroll.0 + pos.0 + 3),
            f(oy + scroll.1 + pos.1),
            26.0,
            26.0,
            icon_tint,
        );
        batch.text_styled(atlas, &Text::literal(title), f(ox + plan.title_x), f(oy + plan.title_y), u32::MAX, true);
        if let Some(p) = &progress {
            batch.text_styled(atlas, &Text::literal(p.clone()), f(ox + plan.progress_x), f(oy + plan.title_y), u32::MAX, true);
        }
        for (n, line) in desc.drain(..).enumerate() {
            batch.text_styled(
                atlas,
                &line,
                f(ox + plan.desc_x),
                f(oy + plan.desc_y + n as i32 * engine::LINE_HEIGHT),
                0xFFAA_AAAA,
                false,
            );
        }
        if show && grid.has_pages() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64);
            let page = grid.current_page(now, cfg.criteria_rotation_speed);
            let mut x = plan.grid_x;
            for column in &grid.paged[page.min(grid.paged.len() - 1)] {
                for (row, &cell) in column.cells.iter().enumerate() {
                    let (marker, body) = &cell_texts[cell];
                    let c = cells.as_ref().map(|c| c[cell].clone()).expect("cells");
                    let y = f(oy + plan.grid_y + row as i32 * engine::LINE_HEIGHT);
                    batch.text_styled(atlas, marker, f(ox + x), y, rgb_of(c.marker_color), false);
                    let mw = font.width_text(marker);
                    batch.text_styled(atlas, body, f(ox + x) + mw, y, rgb_of(c.body_color), false);
                }
                x += column.width;
            }
        }
        self.draw_item(
            batch,
            &ItemStack::new(&d.icon, 1),
            f(ox + scroll.0 + pos.0 + 8),
            f(oy + scroll.1 + pos.1 + 5),
        );
    }

    /// A left button press or release (the hook of `advancements_mouse`). True when the mod handled it.
    pub(super) fn betteradvancements_mouse(&mut self, pressed: bool, size: [u32; 2]) {
        if !pressed {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.drag = None;
                s.tabs.scrolling = false;
            });
            return;
        }
        let cfg = self.betteradvancements_config();
        let view = self.betteradvancements_view(&cfg);
        let b = HudBatch::new(size);
        let w = engine::Window::new(b.width as i32, b.height as i32, cfg.ui_scaling);
        let (width, height) = w.render_size();
        let per_page = engine::max_tabs(width, height, cfg.only_use_above_tabs);
        let (cx, cy) = self.cursor;
        // The page buttons.
        let paging = STATE.with(|s| s.borrow().paging);
        if paging.max_pages != 0 {
            for (n, (x, y, bw, bh)) in engine::page_buttons(&w).into_iter().enumerate() {
                if ui::hovered(self.cursor, x as f32, y as f32, bw as f32, bh as f32) {
                    self.audio.play("minecraft:ui.button.click", None, 0.25, 1.0);
                    STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        if n == 0 { s.paging.previous() } else { s.paging.next() }
                    });
                    return;
                }
            }
        }
        // The tabs (`mouseClicked`: the hit size is the click one).
        let (cw, ch) = w.click_size();
        for ti in paging.range(per_page, view.tabs.len()) {
            let global = ti as i32;
            let Some(side) = engine::tab_side(width, height, global, cfg.only_use_above_tabs) else { continue };
            if engine::tab_over(side, w.left, w.top, cw, ch, global, f64::from(cx), f64::from(cy)) {
                let root = self.adv.tree.list[view.tabs[ti]].id.clone();
                let bounds = self.betteradvancements_bounds(view.tabs[ti]);
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    if s.tabs.selected.as_ref() != Some(&root) {
                        let State { tabs, history, .. } = &mut *s;
                        tabs.select(&root, &bounds, history, &w);
                    }
                });
                return;
            }
        }
        STATE.with(|s| s.borrow_mut().drag = Some((cx, cy)));
    }

    /// The bounds of a tab's widgets, for a tab that is not the drawn one.
    fn betteradvancements_bounds(&self, root: usize) -> engine::Bounds {
        STATE.with(|s| {
            let s = s.borrow();
            let Some(c) = s.cache.as_ref() else { return engine::Bounds::of(&[]) };
            let mut positions = Vec::new();
            let mut stack = vec![root];
            while let Some(i) = stack.pop() {
                if c.tree.list[i].display.is_some()
                    && let Some(p) = c.pos[i]
                {
                    positions.push(p);
                }
                stack.extend(c.kids[i].iter().copied());
            }
            engine::Bounds::of(&positions)
        })
    }

    /// `mouseDragged` (left button).
    pub(super) fn betteradvancements_drag(&mut self) {
        let Some(last) = STATE.with(|s| s.borrow().drag) else { return };
        let (dx, dy) = (self.cursor.0 - last.0, self.cursor.1 - last.1);
        STATE.with(|s| s.borrow_mut().drag = Some(self.cursor));
        let moving = STATE.with(|s| s.borrow_mut().tabs.drag_step());
        if moving {
            self.betteradvancements_scroll_by(f64::from(dx), f64::from(dy));
        }
    }

    pub(super) fn betteradvancements_scroll_by(&mut self, dx: f64, dy: f64) {
        let size = self.window.as_ref().map_or([1280, 720], |w| w.inner_size().into());
        let cfg = self.betteradvancements_config();
        let view = self.betteradvancements_view(&cfg);
        let b = HudBatch::new(size);
        let w = engine::Window::new(b.width as i32, b.height as i32, cfg.ui_scaling);
        STATE.with(|s| s.borrow_mut().tabs.scroll_by(&view.bounds, dx, dy, &w));
    }
}
