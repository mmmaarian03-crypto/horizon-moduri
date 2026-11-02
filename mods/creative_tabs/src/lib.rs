// Creative tabs of content mods for Horizon: the rules, without any game code.
// Written from how Forge 1.20.1's creative inventory lays tabs out (one tab per mod, the vanilla
// tabs on the first page, "<" and ">" buttons and a "page / pages" label when the tabs do not fit),
// and from the pack (the lang keys `itemGroup.*` of the 1.20.1 jars, read only for the titles);
// no code of Forge or of any mod is used.
use std::collections::{BTreeMap, HashMap, HashSet};

/// Tabs of one page that are not right-aligned: five in the top row, five in the bottom row
/// (columns 0 to 4). Search and the survival inventory (columns 5 and 6) stay on every page.
pub const TABS_PER_PAGE: usize = 10;
/// Columns of one row that paged tabs take.
pub const ROW_COLUMNS: usize = 5;
/// `CreativeModeInventoryScreen.imageWidth`.
pub const PANEL_WIDTH: f32 = 195.0;
/// Forge 1.20.1's "<" and ">" buttons are 20 by 20, 50 above the screen's top edge, one at each
/// side; the "page / pages" label is centred 44 above it (from memory of the patched screen).
pub const ARROW_SIZE: f32 = 20.0;
pub const ARROW_TOP: f32 = -50.0;
pub const LABEL_TOP: f32 = -44.0;
/// Where `Creative::tab_position` puts a tab that is on another page: nothing there can be
/// pointed at or drawn on screen, and the screen's own loops need no change.
pub const OFF_SCREEN: (f32, f32) = (-30000.0, -30000.0);

/// Tab icons for mods whose 1.20.1 tab shows an obvious namesake item. These two are NOT checked
/// against the originals (their tab classes are not read): a hint is used only when the mod has
/// that item, else the tab shows its first item.
pub const ICON_HINTS: &[(&str, &str)] = &[
    ("ironchest", "ironchest:iron_chest"),
    ("torchmaster", "torchmaster:megatorch"),
];

// ---------------------------------------------------------------------------------------------
// The registry: the keys of `items.json`, in file order.

/// The keys of the top-level object of a JSON document, in file order, without reading the values
/// (the content root's `items.json` is megabytes of per-item components). Stops at the first
/// syntax it cannot follow and returns what it has.
pub fn top_level_keys(json: &[u8]) -> Vec<String> {
    let b = json.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(json);
    let mut keys = Vec::new();
    let mut i = 0;
    skip_space(b, &mut i);
    if b.get(i) != Some(&b'{') {
        return keys;
    }
    i += 1;
    loop {
        skip_space(b, &mut i);
        if b.get(i) != Some(&b'"') {
            return keys;
        }
        let Some(key) = read_string(b, &mut i) else {
            return keys;
        };
        skip_space(b, &mut i);
        if b.get(i) != Some(&b':') {
            return keys;
        }
        i += 1;
        skip_space(b, &mut i);
        if !skip_value(b, &mut i) {
            return keys;
        }
        keys.push(key);
        skip_space(b, &mut i);
        match b.get(i) {
            Some(b',') => i += 1,
            _ => return keys,
        }
    }
}

fn skip_space(b: &[u8], i: &mut usize) {
    while matches!(b.get(*i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        *i += 1;
    }
}

/// The string starting at the quote `b[*i]`; leaves `i` after the closing quote.
fn read_string(b: &[u8], i: &mut usize) -> Option<String> {
    let start = *i + 1;
    let mut j = start;
    while j < b.len() && b[j] != b'"' && b[j] != b'\\' {
        j += 1;
    }
    match b.get(j)? {
        b'"' => {
            *i = j + 1;
            Some(String::from_utf8_lossy(&b[start..j]).into_owned())
        }
        _ => {
            // Escapes: decode into bytes, then UTF-8.
            let mut out = b[start..j].to_vec();
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        *i = j + 1;
                        return Some(String::from_utf8_lossy(&out).into_owned());
                    }
                    b'\\' => {
                        let c = *b.get(j + 1)?;
                        j += 2;
                        let plain = match c {
                            b'n' => Some('\n'),
                            b't' => Some('\t'),
                            b'r' => Some('\r'),
                            b'b' => Some('\u{8}'),
                            b'f' => Some('\u{c}'),
                            b'"' | b'\\' | b'/' => Some(c as char),
                            b'u' => None,
                            _ => return None,
                        };
                        let ch = match plain {
                            Some(ch) => ch,
                            None => {
                                let mut unit = hex4(b.get(j..j + 4)?)?;
                                j += 4;
                                if (0xD800..0xDC00).contains(&unit)
                                    && b.get(j) == Some(&b'\\')
                                    && b.get(j + 1) == Some(&b'u')
                                {
                                    let low = hex4(b.get(j + 2..j + 6)?)?;
                                    if (0xDC00..0xE000).contains(&low) {
                                        unit = 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
                                        j += 6;
                                    }
                                }
                                char::from_u32(unit).unwrap_or('\u{FFFD}')
                            }
                        };
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    byte => {
                        out.push(byte);
                        j += 1;
                    }
                }
            }
            None
        }
    }
}

fn hex4(digits: &[u8]) -> Option<u32> {
    let text = std::str::from_utf8(digits).ok()?;
    if text.len() != 4 {
        return None;
    }
    u32::from_str_radix(text, 16).ok()
}

/// Moves `i` past the string starting at the quote `b[*i]`.
fn skip_string(b: &[u8], i: &mut usize) -> bool {
    let mut j = *i + 1;
    while j < b.len() {
        match b[j] {
            b'"' => {
                *i = j + 1;
                return true;
            }
            b'\\' => j += 2,
            _ => j += 1,
        }
    }
    false
}

/// Moves `i` past one JSON value (a string, an object or array, or a bare scalar).
fn skip_value(b: &[u8], i: &mut usize) -> bool {
    match b.get(*i) {
        None => false,
        Some(b'"') => skip_string(b, i),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            while *i < b.len() {
                match b[*i] {
                    b'"' => {
                        if !skip_string(b, i) {
                            return false;
                        }
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            *i += 1;
                            return true;
                        }
                    }
                    _ => {}
                }
                *i += 1;
            }
            false
        }
        Some(_) => {
            let start = *i;
            while *i < b.len() && !matches!(b[*i], b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            {
                *i += 1;
            }
            *i > start
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Items into tabs.

/// An installed content mod, from its `horizon-mod.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModInfo {
    pub id: String,
    /// The display name (`name = "Macaw's Doors"`).
    pub name: String,
    /// Its id first, then the namespaces its manifest lists.
    pub namespaces: Vec<String>,
}

/// The items of one mod, in registry order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The mod's id (or, for a namespace no manifest claims, the namespace).
    pub key: String,
    /// The manifest's display name; empty when no manifest was found.
    pub name: String,
    /// The namespaces whose lang keys name the group, the key first.
    pub namespaces: Vec<String>,
    pub items: Vec<String>,
}

pub fn namespace_of(id: &str) -> &str {
    id.split_once(':').map_or("minecraft", |(ns, _)| ns)
}

/// Splits the registry (`ids`, vanilla's first) into the content mods' groups. `content` are the
/// namespaces the root says content mods own; everything else (vanilla, unknown namespaces) is not
/// a mod's. Items keep the registry's order within a mod; groups come in mod id order.
pub fn group_items(ids: &[String], content: &[String], mods: &[ModInfo]) -> Vec<Group> {
    let content: HashSet<&str> = content.iter().map(String::as_str).collect();
    let owner = |ns: &str| mods.iter().find(|m| m.id == ns || m.namespaces.iter().any(|n| n == ns));
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for id in ids {
        let ns = namespace_of(id);
        if ns == "minecraft" || !content.contains(ns) || !seen.insert(id.as_str()) {
            continue;
        }
        let key = owner(ns).map_or_else(|| ns.to_owned(), |m| m.id.clone());
        let group = groups.entry(key.clone()).or_insert_with(|| match owner(ns) {
            Some(m) => Group {
                key: key.clone(),
                name: m.name.clone(),
                namespaces: m.namespaces.clone(),
                items: Vec::new(),
            },
            None => Group {
                key: key.clone(),
                name: String::new(),
                namespaces: vec![ns.to_owned()],
                items: Vec::new(),
            },
        });
        group.items.push(id.clone());
    }
    groups.into_values().collect()
}

/// Lang keys that name a creative tab: `itemGroup.*` (Forge 1.20.1 and vanilla), `item_group.*`
/// and `creativetab.*` (some mods).
pub fn is_group_key(key: &str) -> bool {
    key.starts_with("itemGroup.") || key.starts_with("item_group.") || key.starts_with("creativetab.")
}

const GROUP_PREFIXES: [&str; 3] = ["itemGroup.", "item_group.", "creativetab."];
/// Sub-keys of `itemGroup.<ns>.` that name the mod's main tab, best first.
const MAIN_SUFFIXES: [&str; 5] = ["main", "general", "tab", "base", "default"];
/// Sub-keys that are not a tab's title.
const NOT_TITLES: [&str; 4] = ["no_access", "tooltip", "description", "desc"];

/// A tab's title: the mod's own `itemGroup.<ns>` key; else its only `itemGroup.<ns>.<name>` key
/// (or the main one of several); else the manifest's display name; else the id made readable.
/// `lang` holds the content namespaces' en_us group keys.
pub fn group_title(g: &Group, lang: &BTreeMap<String, String>) -> String {
    let clean = |v: &String| {
        let t = v.trim();
        (!t.is_empty()).then(|| t.to_owned())
    };
    let mut namespaces: Vec<&str> = vec![g.key.as_str()];
    namespaces.extend(g.namespaces.iter().map(String::as_str).filter(|n| *n != g.key));
    for ns in &namespaces {
        for prefix in GROUP_PREFIXES {
            if let Some(t) = lang.get(&format!("{prefix}{ns}")).and_then(clean) {
                return t;
            }
        }
    }
    for ns in &namespaces {
        let mut subs: Vec<(String, String)> = Vec::new();
        for prefix in GROUP_PREFIXES {
            let start = format!("{prefix}{ns}.");
            for (k, v) in lang {
                let Some(suffix) = k.strip_prefix(&start) else {
                    continue;
                };
                if NOT_TITLES.iter().any(|n| suffix.contains(n)) {
                    continue;
                }
                if let Some(t) = clean(v) {
                    subs.push((suffix.to_owned(), t));
                }
            }
        }
        let distinct: HashSet<&str> = subs.iter().map(|(_, t)| t.as_str()).collect();
        if distinct.len() == 1 {
            return subs[0].1.clone();
        }
        for main in MAIN_SUFFIXES {
            if let Some((_, t)) = subs.iter().find(|(s, _)| s == main) {
                return t.clone();
            }
        }
    }
    if !g.name.trim().is_empty() {
        return g.name.trim().to_owned();
    }
    pretty_name(&g.key)
}

/// `mr_dungeons_andtaverns` as `Mr Dungeons Andtaverns`.
pub fn pretty_name(key: &str) -> String {
    key.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map(|f| f.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A tab's icon: the hinted namesake item when the mod has it, else the mod's first item.
pub fn group_icon(g: &Group, hints: &[(&str, &str)]) -> String {
    hints
        .iter()
        .find(|(key, _)| *key == g.key)
        .map(|(_, item)| (*item).to_owned())
        .filter(|item| g.items.iter().any(|i| i == item))
        .or_else(|| g.items.first().cloned())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// Where tabs go: pages.

/// Where a mod's tab sits: its page (0 is vanilla's) and its place in the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub page: usize,
    pub top: bool,
    pub column: u32,
}

/// The slot of the `index`th mod tab: pages from 1, five in the top row then five in the bottom.
pub fn slot_of(index: usize) -> Slot {
    let place = index % TABS_PER_PAGE;
    Slot {
        page: 1 + index / TABS_PER_PAGE,
        top: place < ROW_COLUMNS,
        column: (place % ROW_COLUMNS) as u32,
    }
}

/// Pages for `mod_tabs` tabs of mods: vanilla's page, then ten tabs per page.
pub fn page_count(mod_tabs: usize) -> usize {
    1 + mod_tabs.div_ceil(TABS_PER_PAGE)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedTab {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub items: Vec<String>,
    pub slot: Slot,
}

pub fn tab_id(key: &str) -> String {
    format!("{key}:creative_tab")
}

/// The tabs of all content mods: titles, icons, items and slots.
pub fn plan_tabs(
    ids: &[String],
    content: &[String],
    mods: &[ModInfo],
    lang: &BTreeMap<String, String>,
    hints: &[(&str, &str)],
) -> Vec<PlannedTab> {
    group_items(ids, content, mods)
        .into_iter()
        .enumerate()
        .map(|(i, g)| PlannedTab {
            id: tab_id(&g.key),
            title: group_title(&g, lang),
            icon: group_icon(&g, hints),
            slot: slot_of(i),
            items: g.items,
        })
        .collect()
}

/// A tab the creative screen already has.
#[derive(Clone, Copy, Debug)]
pub struct TabMeta<'a> {
    pub id: &'a str,
    pub column: u32,
    pub search: bool,
    pub inventory: bool,
}

/// The tabs that stay on every page: search, the survival inventory and anything right-aligned.
pub fn is_fixed(t: &TabMeta) -> bool {
    t.search || t.inventory || t.column as usize >= ROW_COLUMNS
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merge {
    /// Where the mod tabs go in the screen's tab list: after the last paged tab it has.
    pub insert_at: usize,
    /// The page of every tab that is not fixed.
    pub tab_page: BTreeMap<String, usize>,
    pub pages: usize,
}

/// Places the planned tabs among the screen's `existing` ones. With no planned tab nothing
/// changes: one page, every tab visible, as vanilla.
pub fn merge_layout(existing: &[TabMeta], planned: &[PlannedTab]) -> Merge {
    if planned.is_empty() {
        return Merge {
            insert_at: existing.len(),
            tab_page: BTreeMap::new(),
            pages: 1,
        };
    }
    let mut tab_page = BTreeMap::new();
    let mut insert_at = 0;
    for (i, t) in existing.iter().enumerate() {
        if !is_fixed(t) {
            tab_page.insert(t.id.to_owned(), 0);
            insert_at = i + 1;
        }
    }
    for p in planned {
        tab_page.insert(p.id.clone(), p.slot.page);
    }
    Merge {
        insert_at,
        tab_page,
        pages: page_count(planned.len()),
    }
}

/// The index of the tab that was at `selected` after the mod tabs are inserted at `insert_at`
/// (`added` of them): tabs from there on move back.
pub fn selected_after(selected: usize, insert_at: usize, added: usize) -> usize {
    if selected >= insert_at {
        selected + added
    } else {
        selected
    }
}

/// Which page shows which tabs, and the page the screen is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paging {
    pages: usize,
    page: usize,
    tab_page: BTreeMap<String, usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Previous,
    Next,
}

impl Paging {
    pub const fn empty() -> Self {
        Self {
            pages: 1,
            page: 0,
            tab_page: BTreeMap::new(),
        }
    }
    pub fn new(merge: &Merge) -> Self {
        Self {
            pages: merge.pages.max(1),
            page: 0,
            tab_page: merge.tab_page.clone(),
        }
    }
    pub fn pages(&self) -> usize {
        self.pages
    }
    pub fn page(&self) -> usize {
        self.page
    }
    /// Whether the arrows and the label are shown.
    pub fn paged(&self) -> bool {
        self.pages > 1
    }
    /// Whether the tab is drawn and can be pointed at on the current page.
    pub fn visible(&self, id: &str) -> bool {
        self.tab_page.get(id).is_none_or(|p| *p == self.page)
    }
    /// Goes to `page` (kept within the pages); whether it changed.
    pub fn set_page(&mut self, page: usize) -> bool {
        let page = page.min(self.pages - 1);
        let changed = page != self.page;
        self.page = page;
        changed
    }
    /// One page back (negative) or forward, stopping at the first and the last.
    pub fn step(&mut self, delta: i32) -> bool {
        let to = (self.page as i64 + i64::from(delta)).clamp(0, self.pages as i64 - 1);
        self.set_page(to as usize)
    }
    /// Goes to the page of a tab that is on another one; fixed and unknown tabs leave it.
    pub fn show(&mut self, id: &str) -> bool {
        match self.tab_page.get(id) {
            Some(&page) => self.set_page(page),
            None => false,
        }
    }
    /// The label between the arrows: `1 / 3`.
    pub fn label(&self) -> String {
        format!("{} / {}", self.page + 1, self.pages)
    }
    /// The arrow under a point, relative to the screen's top-left corner; none when not paged.
    pub fn arrow_at(&self, x: f32, y: f32) -> Option<Arrow> {
        if !self.paged() {
            return None;
        }
        [Arrow::Previous, Arrow::Next]
            .into_iter()
            .find(|a| inside(arrow_rect(*a), x, y))
    }
    /// A click at a point relative to the screen: an arrow turns the page (at the first or last
    /// page it does nothing, but the click is still the button's). Whether an arrow took it.
    pub fn press(&mut self, x: f32, y: f32) -> bool {
        match self.arrow_at(x, y) {
            Some(Arrow::Previous) => {
                self.step(-1);
                true
            }
            Some(Arrow::Next) => {
                self.step(1);
                true
            }
            None => false,
        }
    }
}

/// A button's `[x, y, width, height]` relative to the screen's top-left corner.
pub fn arrow_rect(a: Arrow) -> [f32; 4] {
    match a {
        Arrow::Previous => [0.0, ARROW_TOP, ARROW_SIZE, ARROW_SIZE],
        Arrow::Next => [PANEL_WIDTH - ARROW_SIZE, ARROW_TOP, ARROW_SIZE, ARROW_SIZE],
    }
}

fn inside([rx, ry, w, h]: [f32; 4], x: f32, y: f32) -> bool {
    x >= rx && y >= ry && x < rx + w && y < ry + h
}

/// The label's left edge, relative to the screen, for a text of `width` (drawCenteredString).
pub fn label_x(width: f32) -> f32 {
    ((PANEL_WIDTH - width) / 2.0).floor()
}

// ---------------------------------------------------------------------------------------------
// The search tab.

/// What the search tab indexes for one mod item: the item and its item tags (`ns:path`), so a
/// `#tag` search finds it as it finds vanilla's items. The name comes from the lang files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRow {
    pub item: String,
    pub tags: Vec<String>,
}

/// The mod items the search tab lists, tab by tab in the tabs' order, with their tags.
/// `tags` maps a tag id to the items it holds (nested tags already resolved).
pub fn search_rows(planned: &[PlannedTab], tags: &HashMap<String, HashSet<String>>) -> Vec<SearchRow> {
    let mut of_item: HashMap<&str, Vec<String>> = HashMap::new();
    let mut names: Vec<&String> = tags.keys().collect();
    names.sort();
    let wanted: HashSet<&str> = planned
        .iter()
        .flat_map(|p| p.items.iter().map(String::as_str))
        .collect();
    for name in names {
        for item in &tags[name] {
            if wanted.contains(item.as_str()) {
                of_item.entry(item.as_str()).or_default().push(name.clone());
            }
        }
    }
    planned
        .iter()
        .flat_map(|p| p.items.iter())
        .map(|item| SearchRow {
            item: item.clone(),
            tags: of_item.get(item.as_str()).cloned().unwrap_or_default(),
        })
        .collect()
}
