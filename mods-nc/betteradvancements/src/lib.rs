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
