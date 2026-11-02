// Patchouli for Horizon: the rules, without any game code.
// Written from the 1.20.1 mod (Vazkii/Patchouli, branch 1.20.1, build 86; the pack plays 85) and the pack's config.
// This file is pasted into the game as `crate::game::patchouli::engine` (tools/hnmod.py bundle) and is also the
// `horizon_patchouli` crate the tests use: std + serde_json, no game types, no inner attributes (`#![...]`).
//
// What is here (all of it plain data in, plain data out, so it is tested without the game):
// - the book model and its loading (`book.json`, `categories/`, `entries/`, `templates/`, languages);
// - the text markup (`$(...)` commands, macros, links) and its layout (`TextLayouter`, overflow modes);
// - page templates (components, variables, inclusions) and the recipe pages;
// - the reading state (viewed entries, bookmarks, history, quests) and the navigation (`GuiBook*` screens);
// - the view: a list of drawing elements in book coordinates, which the game's side draws.
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub type Pos = [i32; 3];

/// Valorile pachetului (profilul the 1.20.1 pack/config), generate de tools/nou_mod.py.
/// Păstrează doar ce folosește portul; scrie sursa fiecărei valori folosite.
#[allow(dead_code)]
pub mod config {
    // ---- patchouli-client.toml
    /// Set this to true to disable advancement locking for ALL books, making all entries visible at all times. Config Flag: advancements_disabled
    pub const DISABLE_ADVANCEMENT_LOCKING: bool = false; // patchouli-client.toml:2 [] disableAdvancementLocking
    /// Granular list of Book ID's to disable advancement locking for, e.g. [ "botania:lexicon" ]. Config Flags: advancements_disabled_<bookid>
    pub const NO_ADVANCEMENT_BOOKS: &[&str] = &[]; // patchouli-client.toml:4 [] noAdvancementBooks
    /// Enable testing mode. By default this doesn't do anything, but you can use the config flag in your books if you want. Config Flag: testing_mode
    pub const TESTING_MODE: bool = false; // patchouli-client.toml:6 [] testingMode
    /// Set this to the ID of a book to have it show up in players' inventories, replacing the recipe book.
    pub const INVENTORY_BUTTON_BOOK: &str = ""; // patchouli-client.toml:8 [] inventoryButtonBook
    /// Set this to true to use Shift instead of Ctrl for the inventory quick lookup feature.
    pub const USE_SHIFT_FOR_QUICK_LOOKUP: bool = false; // patchouli-client.toml:10 [] useShiftForQuickLookup
    /// Set how text overflow should be coped with: overflow the text off the page, truncate overflowed text, or resize everything to fit. Relogin after changing. Allowed Values: OVERFLOW,
    pub const TEXT_OVERFLOW_MODE: &str = "RESIZE"; // patchouli-client.toml:13 [] textOverflowMode
    /// How long in ticks the quick lookup key needs to be pressed before the book opens
    pub const QUICK_LOOKUP_TIME: i64 = 10; // patchouli-client.toml:15 [] quickLookupTime
}

/// `GuiBook` constants (GuiBook.java, GuiBookEntryList.java).
pub const FULL_WIDTH: i32 = 272;
pub const FULL_HEIGHT: i32 = 180;
pub const PAGE_WIDTH: i32 = 116;
pub const PAGE_HEIGHT: i32 = 156;
pub const TOP_PADDING: i32 = 18;
pub const LEFT_PAGE_X: i32 = 15;
pub const RIGHT_PAGE_X: i32 = 141;
pub const TEXT_LINE_HEIGHT: i32 = 9;
pub const MAX_BOOKMARKS: usize = 10;
pub const ENTRIES_PER_PAGE: usize = 13;
pub const ENTRIES_IN_FIRST_PAGE: usize = 11;
/// `BookContentsBuilder.DEFAULT_LANG`.
pub const DEFAULT_LANG: &str = "en_us";

// ------------------------------------------------------------------------------------------------
// JSON (Gson reads leniently: comments and trailing commas are accepted)
// ------------------------------------------------------------------------------------------------

/// Parses a book file. `Gson.fromJson(Reader)` is lenient, so `//` and `/* */` comments and a comma before
/// `}` or `]` are tolerated; the BOM is dropped.
pub fn parse_json(text: &str) -> Result<Value, String> {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    match serde_json::from_str::<Value>(t) {
        Ok(v) => Ok(v),
        Err(first) => serde_json::from_str::<Value>(&clean_json(t)).map_err(|_| first.to_string()),
    }
}

fn clean_json(t: &str) -> String {
    let chars: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    let (mut i, mut in_string, mut escaped) = (0, false, false);
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if chars.get(i + 1) == Some(&'*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            ',' => {
                let mut j = i + 1;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if !matches!(chars.get(j), Some('}') | Some(']')) {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
        i += 1;
    }
    out
}

pub fn jstr<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
pub fn jbool(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(default)
}
pub fn jint(v: &Value, key: &str, default: i64) -> i64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
        _ => default,
    }
}
pub fn jfloat(v: &Value, key: &str, default: f64) -> f64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(default),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(default),
        _ => default,
    }
}

/// `IVariable.asString`: the JSON value as Gson's `getAsString` gives it (`""` for null/missing).
pub fn as_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// `IVariable.asBoolean`.
pub fn as_bool(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty() && s != "false" && s.eq_ignore_ascii_case("true"),
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        _ => false,
    }
}

/// `ResourceLocation` as `(namespace, path)`; a bare name takes `default_ns`.
pub fn split_id<'a>(default_ns: &'a str, s: &'a str) -> (&'a str, &'a str) {
    match s.split_once(':') {
        Some((n, p)) => (n, p),
        None => (default_ns, s),
    }
}
pub fn qualify(default_ns: &str, s: &str) -> String {
    let (n, p) = split_id(default_ns, s);
    format!("{n}:{p}")
}

// ------------------------------------------------------------------------------------------------
// Item stacks as the books write them (ItemStackUtil)
// ------------------------------------------------------------------------------------------------

/// An item stack: `ns:item#count{nbt}`. The count and the NBT come from the book; the game's side decides
/// whether the item exists (a book may name items of a mod that is not installed).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Stack {
    pub id: String,
    pub count: u32,
    pub nbt: Option<String>,
}
impl Stack {
    pub fn new(id: &str, count: u32) -> Self {
        Self { id: id.to_owned(), count, nbt: None }
    }
    pub fn is_empty(&self) -> bool {
        self.id.is_empty() || self.id == "minecraft:air" || self.count == 0
    }
}

/// `ItemStackUtil.parseItemStackString`.
pub fn parse_stack(res: &str) -> Result<Stack, String> {
    let mut res = res.trim().to_owned();
    let mut nbt = None;
    if let Some(start) = res.find('{') {
        if start > 0 {
            let raw = res[start..].to_owned();
            nbt = Some(unquote_nbt(&raw));
            res.truncate(start);
        }
    }
    let mut count = 1u32;
    if let Some((head, tail)) = res.split_once('#') {
        count = tail.trim().parse::<u32>().map_err(|_| format!("Bad item count in {res}"))?;
        res = head.to_owned();
    }
    let mut tokens = res.split(':');
    let (Some(ns), Some(path)) = (tokens.next(), tokens.next()) else {
        return Err(format!("Malformed item ID {res}"));
    };
    if ns.is_empty() || path.is_empty() {
        return Err(format!("Malformed item ID {res}"));
    }
    Ok(Stack { id: format!("{ns}:{path}"), count, nbt })
}

/// Patchouli writes NBT with single quotes inside JSON strings: `([^\\])'` becomes `"` and `\'` becomes `'`.
fn unquote_nbt(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'\'') {
            out.push('\'');
            i += 2;
            continue;
        }
        if chars[i] == '\'' && i > 0 && chars[i - 1] != '\\' {
            out.push('"');
        } else {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

/// `ItemStackUtil.loadStackListFromString`: stacks separated by commas (outside braces and quotes);
/// `tag:ns:name` is every item of the tag.
pub fn parse_stack_list(env: &dyn Env, s: &str) -> Result<Vec<Stack>, String> {
    let mut parts: Vec<String> = Vec::new();
    let (mut braces, mut quote, mut cur) = (0i32, None::<char>, String::new());
    for c in s.chars() {
        match c {
            '{' if quote.is_none() => braces += 1,
            '}' if quote.is_none() => braces -= 1,
            '\'' | '"' => {
                quote = match quote {
                    None => Some(c),
                    Some(q) if q == c => None,
                    other => other,
                };
            }
            ',' if braces <= 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    let mut out = Vec::new();
    for p in parts {
        let p = p.trim();
        if let Some(tag) = p.strip_prefix("tag:") {
            for item in env.tag_items(&qualify("minecraft", tag)) {
                out.push(Stack::new(&item, 1));
            }
        } else if !p.is_empty() {
            out.push(parse_stack(p)?);
        }
    }
    Ok(out)
}

/// An `Ingredient` or `ItemStack[]` variable: a string (`ItemStackUtil`), a `{"item","count","nbt"}` object, a tag
/// object, or an array of those. Unknown shapes give no stacks.
pub fn stacks_of(env: &dyn Env, v: &Value) -> Vec<Stack> {
    match v {
        Value::Null => Vec::new(),
        Value::String(s) => parse_stack_list(env, s).unwrap_or_default(),
        Value::Array(a) => a.iter().flat_map(|e| stacks_of(env, e)).collect(),
        Value::Object(o) => {
            if let Some(item) = o.get("item").and_then(Value::as_str) {
                let mut s = Stack::new(&qualify("minecraft", item), o.get("count").and_then(Value::as_u64).unwrap_or(1) as u32);
                s.nbt = o.get("nbt").map(|n| match n {
                    Value::String(t) => t.clone(),
                    other => other.to_string(),
                });
                vec![s]
            } else if let Some(tag) = o.get("tag").and_then(Value::as_str) {
                env.tag_items(&qualify("minecraft", tag)).into_iter().map(|i| Stack::new(&i, 1)).collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

// ------------------------------------------------------------------------------------------------
// What the engine asks of its surroundings
// ------------------------------------------------------------------------------------------------

/// The resource root (`<root>/assets/...`, `<root>/data/...`): where the books are.
pub trait Files {
    /// A file's text; `None` when it does not exist.
    fn read(&self, path: &str) -> Option<String>;
    /// The names directly inside a folder (files and folders alike).
    fn list(&self, dir: &str) -> Vec<String>;
}

/// The files of a folder on disk.
pub struct FsFiles(pub std::path::PathBuf);
impl Files for FsFiles {
    fn read(&self, path: &str) -> Option<String> {
        let bytes = std::fs::read(self.0.join(path)).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(self.0.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        out.sort();
        out
    }
}

/// An in-memory tree of files (tests, and the books of a fake world).
#[derive(Default)]
pub struct MemFiles(pub BTreeMap<String, String>);
impl Files for MemFiles {
    fn read(&self, path: &str) -> Option<String> {
        self.0.get(path).cloned()
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{}/", dir.trim_end_matches('/'));
        let mut out = BTreeSet::new();
        for k in self.0.keys() {
            if let Some(rest) = k.strip_prefix(&prefix) {
                out.insert(rest.split('/').next().unwrap_or("").to_owned());
            }
        }
        out.into_iter().filter(|s| !s.is_empty()).collect()
    }
}

/// Every file path under `dir`, recursively, relative to it, sorted.
pub fn walk(files: &dyn Files, dir: &str) -> Vec<String> {
    fn go(files: &dyn Files, dir: &str, rel: &str, out: &mut Vec<String>, depth: usize) {
        if depth > 12 {
            return;
        }
        for name in files.list(dir) {
            let path = format!("{dir}/{name}");
            let rel_path = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
            if name.ends_with(".json") {
                out.push(rel_path);
            } else if !name.contains('.') {
                go(files, &path, &rel_path, out, depth + 1);
            }
        }
    }
    let mut out = Vec::new();
    go(files, dir, "", &mut out, 0);
    out.sort();
    out
}

/// What the game gives the books: translations, items, tags, advancements, recipes, the font.
pub trait Env {
    /// The language code of the player (`en_us`); books fall back to `en_us` for a missing file.
    fn lang_code(&self) -> String;
    /// `I18n.get`: `None` when the key has no text (the key itself is then shown).
    fn translate(&self, key: &str) -> Option<String>;
    /// The item exists in this game.
    fn item_exists(&self, id: &str) -> bool;
    /// `ItemStack.getHoverName`.
    fn item_name(&self, stack: &Stack) -> String;
    /// The items of an item tag (`minecraft:logs`), in order; empty when the tag is unknown.
    fn tag_items(&self, tag: &str) -> Vec<String>;
    /// `ClientAdvancements.hasDone`.
    fn advancement_done(&self, id: &str) -> bool;
    /// `PatchouliConfig` flag `mod:<ns>`: the namespace is installed.
    fn mod_loaded(&self, ns: &str) -> bool;
    /// A recipe's JSON by id (`minecraft:stone_bricks`, `ns:path`).
    fn recipe(&self, id: &str) -> Option<Value>;
    /// A character's advance in the font (`bold` adds the bold offset); `uniform` is the Unicode font that books use
    /// unless `use_blocky_font`.
    fn advance(&self, c: char, bold: bool, uniform: bool) -> f32;
    /// `$(playername)`.
    fn player_name(&self) -> String;
    /// `$(k:name)`: the translated name of the key binding and the name of its current key.
    fn keybind(&self, name: &str) -> Option<(String, String)>;
    /// F3+H (advanced tooltips): entry pages show their id.
    fn advanced_tooltips(&self) -> bool {
        false
    }
    /// The display name of an entity type (`minecraft:zombie`), when the game knows it.
    fn entity_name(&self, _id: &str) -> Option<String> {
        None
    }
}

// ------------------------------------------------------------------------------------------------
// Text: the markup (BookTextParser, SpanState, Span) and its layout (TextLayouter, Word, BookTextRenderer)
// ------------------------------------------------------------------------------------------------

/// `ChatFormatting` colours 0-9, a-f.
pub const COLORS: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA, 0x555555, 0x5555FF, 0x55FF55,
    0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

/// A text style: colour (0xRRGGBB) and the five switches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Style {
    pub color: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub obf: bool,
}
impl Style {
    pub fn colored(color: u32) -> Self {
        Self { color, ..Self::default() }
    }
}

/// What a click on a word or a button does.
#[derive(Clone, Debug, PartialEq)]
pub enum Link {
    /// Open an entry at a spread (`GuiBookEntry(book, entry, page)`).
    Entry { id: String, spread: usize },
    Category(String),
    /// `GuiBook.openWebLink`.
    Url(String),
    /// `$(command:/...)`: the text of the command without its slash.
    Command(String),
}

/// A piece of text sharing one style (`Span`).
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub cluster: Option<u32>,
    pub tooltip: String,
    pub link: Option<Link>,
    pub line_breaks: i32,
    pub spacing_left: i32,
    pub spacing_right: i32,
}

/// What the text commands may look at.
pub struct TextCx<'a> {
    pub env: &'a dyn Env,
    pub book: &'a BookInfo,
    pub contents: &'a Contents,
}
impl TextCx<'_> {
    /// `!book.useBlockyFont`: the Unicode font.
    pub fn uniform(&self) -> bool {
        !self.book.use_blocky_font
    }
    /// `I18n.get` when the book is `i18n`, else the text as it is.
    pub fn i18n(&self, s: &str) -> String {
        if self.book.i18n { self.env.translate(s).unwrap_or_else(|| s.to_owned()) } else { s.to_owned() }
    }
    /// `Component.translatable(key)`: the translation, or the key itself.
    pub fn translatable(&self, key: &str) -> String {
        self.env.translate(key).unwrap_or_else(|| key.to_owned())
    }
    pub fn tr(&self, key: &str, fallback: &str) -> String {
        self.env.translate(key).unwrap_or_else(|| fallback.to_owned())
    }
}

/// The width of `s` in the font, in whole pixels (`Font.width`): `§` codes are formatting, not glyphs.
pub fn text_width(env: &dyn Env, s: &str, bold: bool, uniform: bool) -> i32 {
    let (mut bold, mut sum) = (bold, 0.0f32);
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '§' {
            match it.next() {
                Some('l' | 'L') => bold = true,
                Some(d) if d.is_ascii_hexdigit() || matches!(d, 'r' | 'R') => bold = false,
                _ => {}
            }
            continue;
        }
        sum += env.advance(c, bold, uniform);
    }
    sum.ceil() as i32
}

/// The state while a text is read (`SpanState`).
struct SpanState {
    stack: Vec<Style>,
    base: Style,
    tooltip: String,
    link: Option<Link>,
    cluster: Option<u32>,
    is_external_link: bool,
    ending_external: bool,
    line_breaks: i32,
    spacing_left: i32,
    spacing_right: i32,
    space_width: i32,
}
impl SpanState {
    fn peek(&self) -> Style {
        *self.stack.last().unwrap()
    }
    fn modify(&mut self, f: impl FnOnce(&mut Style)) {
        f(self.stack.last_mut().unwrap());
    }
    fn push_color(&mut self, color: u32) {
        let mut s = self.peek();
        s.color = color;
        self.stack.push(s);
    }
    fn pop(&mut self) -> Result<(), ()> {
        if self.stack.len() <= 1 {
            return Err(());
        }
        self.stack.pop();
        Ok(())
    }
    fn reset(&mut self) {
        self.ending_external = self.is_external_link;
        self.stack.clear();
        self.stack.push(self.base);
        self.cluster = None;
        self.tooltip.clear();
        self.link = None;
        self.is_external_link = false;
    }
    /// `new Span(state, text)`: takes the pending line breaks and spacing.
    fn span(&mut self, text: &str) -> Span {
        let s = Span {
            text: text.to_owned(),
            style: self.peek(),
            cluster: self.cluster,
            tooltip: self.tooltip.clone(),
            link: self.link.clone(),
            line_breaks: self.line_breaks,
            spacing_left: self.spacing_left,
            spacing_right: self.spacing_right,
        };
        self.line_breaks = 0;
        self.spacing_left = 0;
        self.spacing_right = 0;
        s
    }
}

/// `BookTextParser.expandMacros`: every macro of the book, up to ten rounds.
pub fn expand_macros(book: &BookInfo, text: &str) -> String {
    let mut cur = text.to_owned();
    for _ in 0..10 {
        let mut next = cur.clone();
        for (k, v) in &book.macros {
            if !k.is_empty() {
                next = next.replace(k.as_str(), v);
            }
        }
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

/// `BookTextParser.parse`: the markup of `text` as spans. `base` is the book's font with the text colour.
pub fn parse_spans(cx: &TextCx<'_>, text: &str, base: Style) -> Vec<Span> {
    let uniform = cx.uniform();
    let mut state = SpanState {
        stack: vec![base],
        base,
        tooltip: String::new(),
        link: None,
        cluster: None,
        is_external_link: false,
        ending_external: false,
        line_breaks: 0,
        spacing_left: 0,
        spacing_right: 0,
        space_width: text_width(cx.env, " ", false, uniform),
    };
    let mut next_cluster = 0u32;
    let text = expand_macros(cx.book, text);
    let mut spans = Vec::new();
    let mut last = 0usize;
    let mut i = 0usize;
    // The pattern is `\$\(([^)]*)\)`.
    while i < text.len() {
        let Some(rel) = text[i..].find("$(") else { break };
        let start = i + rel;
        let Some(close_rel) = text[start + 2..].find(')') else { break };
        let close = start + 2 + close_rel;
        let cmd = &text[start + 2..close];
        spans.push(state.span(&text[last..start]));
        let processed = process_command(cx, &mut state, &mut next_cluster, cmd);
        if !processed.is_empty() {
            spans.push(state.span(&processed));
            if state.cluster.is_none() {
                state.tooltip.clear();
            }
        }
        last = close + 1;
        i = close + 1;
    }
    spans.push(state.span(&text[last..]));
    spans
}

fn process_command(cx: &TextCx<'_>, st: &mut SpanState, next_cluster: &mut u32, cmd: &str) -> String {
    st.ending_external = false;
    let result = color_code(st, cmd)
        .or_else(|| color_hex(st, cmd))
        .or_else(|| list_command(st, cmd))
        .or_else(|| function_command(cx, st, next_cluster, cmd))
        .or_else(|| plain_command(cx, st, cmd));
    let mut result = result.unwrap_or_else(|| format!("$({cmd})"));
    if st.ending_external {
        result.push_str("§7\u{21AA}");
    }
    result
}

fn color_code(st: &mut SpanState, cmd: &str) -> Option<String> {
    let mut it = cmd.chars();
    let (Some(c), None) = (it.next(), it.next()) else { return None };
    let v = match c {
        '0'..='9' => c as u32 - '0' as u32,
        'a'..='f' => c as u32 - 'a' as u32 + 10,
        _ => return None,
    };
    st.modify(|s| s.color = COLORS[v as usize]);
    Some(String::new())
}

fn color_hex(st: &mut SpanState, cmd: &str) -> Option<String> {
    let hex = cmd.strip_prefix('#')?;
    if hex.len() != 3 && hex.len() != 6 {
        return None;
    }
    let full: String = if hex.len() == 3 { hex.chars().flat_map(|c| [c, c]).collect() } else { hex.to_owned() };
    let color = u32::from_str_radix(&full, 16).unwrap_or(st.base.color);
    st.modify(|s| s.color = color);
    Some(String::new())
}

fn list_command(st: &mut SpanState, cmd: &str) -> Option<String> {
    // `li\d?`
    let rest = cmd.strip_prefix("li")?;
    let mut it = rest.chars();
    let digit = match (it.next(), it.next()) {
        (None, _) => 1,
        (Some(d), None) if d.is_ascii_digit() => d as i32 - '0' as i32,
        _ => return None,
    };
    let pad = digit * 4;
    let bullet = if digit % 2 == 0 { '\u{25E6}' } else { '\u{2022}' };
    st.line_breaks = 1;
    st.spacing_left = pad;
    st.spacing_right = st.space_width;
    Some(format!("§0{bullet}"))
}

fn function_command(cx: &TextCx<'_>, st: &mut SpanState, next_cluster: &mut u32, cmd: &str) -> Option<String> {
    let idx = cmd.find(':').filter(|&i| i > 0)?;
    let (name, param) = (&cmd[..idx], &cmd[idx + 1..]);
    Some(match name {
        "k" => match cx.env.keybind(param) {
            None => {
                st.tooltip = cx.tr("patchouli.gui.lexicon.keybind_missing", "No such keybind: %s").replace("%s", param);
                "N/A".to_owned()
            }
            Some((action, key)) => {
                st.tooltip = cx.tr("patchouli.gui.lexicon.keybind", "Keybind: %s").replace("%s", &action);
                key
            }
        },
        "l" => {
            st.cluster = Some(*next_cluster);
            *next_cluster += 1;
            st.push_color(cx.book.link_color);
            let external = param.starts_with("http:") || param.starts_with("https:");
            if external {
                st.tooltip = cx.tr("patchouli.gui.lexicon.external_link", "(External Link)");
                st.is_external_link = true;
                st.link = Some(Link::Url(param.to_owned()));
            } else {
                let (target, anchor) = match param.find('#') {
                    Some(h) => (&param[..h], Some(&param[h + 1..])),
                    None => (param, None),
                };
                let href = qualify(&cx.book.ns, target);
                if let Some(entry) = cx.contents.entries.get(&href) {
                    st.tooltip = if cx.contents.entry_locked(cx.env, cx.book, &href) {
                        format!("§7{}", cx.tr("patchouli.gui.lexicon.locked", "(Locked)"))
                    } else {
                        cx.i18n(&entry.name)
                    };
                    let mut spread = 0;
                    if let Some(a) = anchor {
                        match entry.page_from_anchor(a) {
                            Some(p) => spread = p / 2,
                            None => st.tooltip.push_str(&format!(" (INVALID ANCHOR:{a})")),
                        }
                    }
                    st.link = Some(Link::Entry { id: href, spread });
                } else if let Some(cat) = cx.contents.categories.get(&href) {
                    if anchor.is_some() {
                        st.tooltip = "BAD LINK: Cannot specify anchor when linking to a category".to_owned();
                    } else {
                        st.tooltip = cx.i18n(&cat.name);
                        st.link = Some(Link::Category(href));
                    }
                } else {
                    st.tooltip = format!("BAD LINK: {target}");
                }
            }
            String::new()
        }
        "tooltip" | "t" => {
            st.tooltip = param.to_owned();
            st.cluster = Some(*next_cluster);
            *next_cluster += 1;
            String::new()
        }
        "command" | "c" => {
            st.push_color(cx.book.link_color);
            st.cluster = Some(*next_cluster);
            *next_cluster += 1;
            if !param.starts_with('/') {
                st.tooltip = "INVALID COMMAND (must begin with /)".to_owned();
            } else {
                let short: String = param.chars().take(20).collect();
                st.tooltip = if param.chars().count() < 20 { param.to_owned() } else { format!("{short}...") };
                st.link = Some(Link::Command(param[1..].to_owned()));
            }
            String::new()
        }
        other => format!("[MISSING FUNCTION: {other}]"),
    })
}

fn plain_command(cx: &TextCx<'_>, st: &mut SpanState, cmd: &str) -> Option<String> {
    match cmd {
        "br" => st.line_breaks = 1,
        "br2" | "2br" | "p" => st.line_breaks = 2,
        "/l" => {
            st.ending_external = st.is_external_link;
            let _ = st.pop();
            st.cluster = None;
            st.tooltip.clear();
            st.link = None;
            st.is_external_link = false;
        }
        "/t" => {
            st.cluster = None;
            st.tooltip.clear();
        }
        "playername" => return Some(cx.env.player_name()),
        "k" | "obf" => st.modify(|s| s.obf = true),
        "l" | "bold" => st.modify(|s| s.bold = true),
        "m" | "strike" => st.modify(|s| s.strike = true),
        "n" | "underline" => st.modify(|s| s.underline = true),
        "o" | "italic" | "italics" => st.modify(|s| s.italic = true),
        "" | "reset" | "clear" => st.reset(),
        "nocolor" => {
            let base = st.base.color;
            st.modify(|s| s.color = base);
        }
        "/c" => {
            let _ = st.pop();
            st.cluster = None;
            st.tooltip.clear();
            st.link = None;
        }
        _ => return None,
    }
    Some(String::new())
}

/// A word placed on a page (`Word`).
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub style: Style,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub link: Option<Link>,
    pub tooltip: String,
    pub cluster: Option<u32>,
}
impl Word {
    pub fn hovered(&self, mx: f64, my: f64) -> bool {
        mx > f64::from(self.x) && my > f64::from(self.y) && mx <= f64::from(self.x + self.width) && my <= f64::from(self.y + self.height)
    }
}

/// How text that does not fit its page is dealt with (`TextOverflowMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    Overflow,
    Truncate,
    Resize,
}
impl Overflow {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "OVERFLOW" => Some(Self::Overflow),
            "TRUNCATE" => Some(Self::Truncate),
            "RESIZE" => Some(Self::Resize),
            _ => None,
        }
    }
}

/// The positions where a line may break (`BreakIterator.getLineInstance`, English): after a run of spaces, after a
/// hyphen between letters or digits, after a dash, around Chinese/Japanese characters.
fn line_boundaries(chars: &[char]) -> Vec<usize> {
    let cjk = |c: char| matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xAC00..=0xD7AF);
    let mut out = vec![0];
    for i in 1..chars.len() {
        let (p, c) = (chars[i - 1], chars[i]);
        let b = (p.is_whitespace() && !c.is_whitespace())
            || (p == '-' && c.is_alphanumeric() && i >= 2 && chars[i - 2].is_alphanumeric())
            || p == '\u{2014}'
            || p == '\u{2013}'
            || p == '\u{200B}'
            || cjk(p)
            || cjk(c);
        if b {
            out.push(i);
        }
    }
    out.push(chars.len());
    out.dedup();
    out
}

/// One piece of a span still to be placed (`SpanTail`).
#[derive(Clone)]
struct Tail {
    span: usize,
    start: usize,
    width: i32,
    length: usize,
}

struct Layouter<'a> {
    env: &'a dyn Env,
    uniform: bool,
    spans: &'a [Span],
    chars: Vec<Vec<char>>,
    page_x: i32,
    page_y: i32,
    line_height: i32,
    base_page_width: i32,
    mode: Overflow,
    words: Vec<Word>,
    y: i32,
    page_width: i32,
    smallest_overstep: i32,
    pending: Vec<Tail>,
    line_start: usize,
    width_so_far: i32,
    bounds: Vec<usize>,
}
impl Layouter<'_> {
    fn tail(&self, span: usize, start: usize) -> Tail {
        let s = &self.spans[span];
        let text: String = self.chars[span][start..].iter().collect();
        let width = text_width(self.env, &text, s.style.bold, self.uniform) + s.spacing_left + s.spacing_right;
        Tail { span, start, width, length: self.chars[span].len() - start }
    }
    fn position(&self, t: &Tail, x: i32, y: i32, length: usize) -> Word {
        let s = &self.spans[t.span];
        let end = (t.start + length).min(self.chars[t.span].len());
        Word {
            text: self.chars[t.span][t.start..end].iter().collect(),
            style: s.style,
            x: x + s.spacing_left,
            y,
            width: t.width,
            height: 8,
            link: s.link.clone(),
            tooltip: s.tooltip.clone(),
            cluster: s.cluster,
        }
    }
    fn layout(&mut self) {
        self.page_width = self.base_page_width;
        let mut guard = 0;
        loop {
            self.y = self.page_y;
            self.words.clear();
            self.smallest_overstep = i32::MAX;
            let mut paragraph: Vec<usize> = Vec::new();
            for i in 0..self.spans.len() {
                if self.spans[i].line_breaks > 0 {
                    self.layout_paragraph(&paragraph);
                    self.width_so_far = 0;
                    self.y += self.spans[i].line_breaks * self.line_height;
                    paragraph.clear();
                }
                paragraph.push(i);
            }
            if !paragraph.is_empty() {
                self.layout_paragraph(&paragraph);
            }
            guard += 1;
            if !(self.mode == Overflow::Resize && self.overflow() * self.scale() > 1.0 && self.adjust_scale() && guard < 64) {
                break;
            }
        }
        if self.mode == Overflow::Truncate {
            let lh = self.line_height;
            self.words.retain(|w| w.y + lh <= PAGE_HEIGHT);
        }
    }
    fn adjust_scale(&mut self) -> bool {
        let by_height = (self.base_page_width as f32 * self.overflow()) as i32;
        self.page_width = 1 + self.smallest_overstep.min(by_height);
        true
    }
    fn scale(&self) -> f32 {
        self.base_page_width as f32 / self.page_width as f32
    }
    fn overflow(&self) -> f32 {
        (self.y + self.line_height - self.page_y) as f32 / (PAGE_HEIGHT - self.page_y) as f32
    }
    fn layout_paragraph(&mut self, paragraph: &[usize]) {
        let all: Vec<char> = paragraph.iter().flat_map(|&i| self.chars[i].iter().copied()).collect();
        self.bounds = line_boundaries(&all);
        self.line_start = 0;
        for &i in paragraph {
            self.layout_span(i);
        }
        self.flush();
    }
    fn layout_span(&mut self, span: usize) {
        let last = self.tail(span, 0);
        self.width_so_far += last.width;
        self.pending.push(last);
        let mut guard = 0;
        while self.width_so_far > self.page_width && guard < 10_000 {
            guard += 1;
            self.break_line();
            self.width_so_far = self.pending.iter().map(|t| t.width).sum();
        }
    }
    fn break_line(&mut self) {
        let mut width: i32 = self.pending.iter().map(|t| t.width).sum();
        let mut offset: usize = self.pending.iter().map(|t| t.length).sum();
        let Some(last) = self.pending.last().cloned() else { return };
        width -= last.width;
        offset -= last.length;
        let span = &self.spans[last.span];
        let (bold, whole) = (span.style.bold, self.chars[last.span].len());
        for i in last.start..whole {
            let c = self.chars[last.span][i];
            width += text_width(self.env, &c.to_string(), bold, self.uniform);
            if bold {
                width += 1;
            }
            if width > self.page_width {
                self.smallest_overstep = self.smallest_overstep.min(width);
                let overflow_offset = self.line_start + offset + i - last.start;
                let mut break_offset = overflow_offset as i64 + 1;
                if !c.is_whitespace() {
                    break_offset = self
                        .bounds
                        .iter()
                        .rev()
                        .find(|&&b| (b as i64) < break_offset)
                        .map_or(-1, |&b| b as i64);
                }
                if break_offset <= self.line_start as i64 {
                    break_offset = overflow_offset as i64 - 1;
                }
                self.break_line_at(break_offset.max(0) as usize);
                return;
            }
        }
        // The two ways of measuring disagree: break here so the book cannot hang.
        self.flush();
        self.y += self.line_height;
    }
    fn break_line_at(&mut self, text_offset: usize) {
        let mut offset = self.line_start;
        let mut x = self.page_x;
        let mut index = 0;
        while index < self.pending.len() {
            let t = self.pending[index].clone();
            if offset + t.length < text_offset {
                let w = self.position(&t, x, self.y, t.length);
                self.words.push(w);
                offset += t.length;
                x += t.width;
            } else {
                let cut = text_offset.saturating_sub(offset);
                let w = self.position(&t, x, self.y, cut);
                self.words.push(w);
                self.pending[index] = self.tail(t.span, t.start + cut);
                break;
            }
            index += 1;
        }
        self.pending.drain(0..index);
        self.line_start = text_offset;
        self.y += self.line_height;
    }
    fn flush(&mut self) {
        let mut x = self.page_x;
        let pending = std::mem::take(&mut self.pending);
        for t in &pending {
            let w = self.position(t, x, self.y, t.length);
            self.words.push(w);
            x += t.width;
        }
    }
}

/// A laid out text (`BookTextRenderer`): its words and the scale of the `RESIZE` mode.
#[derive(Clone, Debug, PartialEq)]
pub struct BookText {
    pub words: Vec<Word>,
    pub scale: f32,
}
impl BookText {
    pub fn empty() -> Self {
        Self { words: Vec::new(), scale: 1.0 }
    }
    /// `new BookTextRenderer(gui, text, x, y, width, lineHeight, baseColor)`; `text` is the book's string (markup).
    pub fn new(cx: &TextCx<'_>, text: &str, x: i32, y: i32, width: i32, line_height: i32, base_color: u32) -> Self {
        let text = if cx.book.i18n { cx.env.translate(text).unwrap_or_else(|| text.to_owned()) } else { text.to_owned() };
        Self::from_markup(cx, &text, x, y, width, line_height, base_color)
    }
    /// The same for text that is already what the player reads (a translated key): no `I18n.get` again.
    pub fn from_markup(cx: &TextCx<'_>, text: &str, x: i32, y: i32, width: i32, line_height: i32, base_color: u32) -> Self {
        let base = Style::colored(base_color);
        let spans = parse_spans(cx, text, base);
        let mode = cx
            .book
            .overflow
            .or_else(|| Overflow::parse(config::TEXT_OVERFLOW_MODE))
            .unwrap_or(Overflow::Overflow);
        let mut l = Layouter {
            env: cx.env,
            uniform: cx.uniform(),
            spans: &spans,
            chars: spans.iter().map(|s| s.text.chars().collect()).collect(),
            page_x: x,
            page_y: y,
            line_height,
            base_page_width: width,
            mode,
            words: Vec::new(),
            y,
            page_width: width,
            smallest_overstep: i32::MAX,
            pending: Vec::new(),
            line_start: 0,
            width_so_far: 0,
            bounds: Vec::new(),
        };
        l.layout();
        let scale = l.scale();
        Self { words: l.words, scale }
    }
    /// Moves every word (the page's place in the book).
    pub fn translate(&mut self, dx: i32, dy: i32) {
        for w in &mut self.words {
            w.x += dx;
            w.y += dy;
        }
    }
    fn rescale(&self, v: f64, origin: f64) -> f64 {
        origin + (v - origin) / f64::from(self.scale)
    }
    fn scaled_mouse(&self, mx: f64, my: f64) -> Option<(f64, f64)> {
        let first = self.words.first()?;
        Some((self.rescale(mx, f64::from(first.x)), self.rescale(my, f64::from(first.y))))
    }
    /// The word under the mouse (book coordinates), if any.
    pub fn word_at(&self, mx: f64, my: f64) -> Option<usize> {
        let (sx, sy) = self.scaled_mouse(mx, my)?;
        self.words.iter().position(|w| w.hovered(sx, sy))
    }
    /// Words that belong to the hovered link (`isClusterHovered`): the whole cluster lights up together.
    pub fn is_hot(&self, word: usize, mx: f64, my: f64) -> bool {
        let Some((sx, sy)) = self.scaled_mouse(mx, my) else { return false };
        let w = &self.words[word];
        match w.cluster {
            None => w.hovered(sx, sy),
            Some(c) => self.words.iter().any(|o| o.cluster == Some(c) && o.hovered(sx, sy)),
        }
    }
    /// `BookTextRenderer.click`: the link of the first word that is clicked.
    pub fn click(&self, mx: f64, my: f64, button: u8) -> Option<Link> {
        if button != 0 {
            return None;
        }
        let (sx, sy) = self.scaled_mouse(mx, my)?;
        self.words.iter().find(|w| w.link.is_some() && w.hovered(sx, sy)).and_then(|w| w.link.clone())
    }
}

// ------------------------------------------------------------------------------------------------
// The book model: Book, BookCategory, BookEntry, BookPage and how they are loaded
// ------------------------------------------------------------------------------------------------

/// `Book` (book.json), without the item and the model.
#[derive(Clone, Debug, PartialEq)]
pub struct BookInfo {
    /// `namespace:name`.
    pub id: String,
    pub ns: String,
    pub name: String,
    pub landing_text: String,
    pub book_texture: String,
    pub filler_texture: String,
    pub crafting_texture: String,
    pub model: String,
    pub use_blocky_font: bool,
    pub text_color: u32,
    pub header_color: u32,
    pub nameplate_color: u32,
    pub link_color: u32,
    pub link_hover_color: u32,
    pub progress_bar_color: u32,
    pub progress_bar_background: u32,
    pub open_sound: String,
    pub flip_sound: String,
    pub show_progress: bool,
    pub index_icon: String,
    pub version: String,
    pub subtitle: String,
    pub creative_tab: Option<String>,
    pub advancements_tab: Option<String>,
    /// `dont_generate_book`: the mod has its own item (`custom_book_item`).
    pub no_book: bool,
    pub custom_book_item: String,
    pub show_toasts: bool,
    pub pause_game: bool,
    pub is_pamphlet: bool,
    pub i18n: bool,
    pub overflow: Option<Overflow>,
    pub macros: BTreeMap<String, String>,
}

fn color_of(v: &Value, key: &str, default: &str) -> u32 {
    let s = jstr(v, key).unwrap_or(default);
    u32::from_str_radix(s.trim().trim_start_matches('#'), 16)
        .or_else(|_| u32::from_str_radix(default, 16))
        .unwrap_or(0)
        & 0xFF_FFFF
}

impl BookInfo {
    /// `new Book(json, owner, id, external)`. Errors are the ones the original throws: the book is then not loaded.
    pub fn from_json(ns: &str, name: &str, v: &Value) -> Result<Self, String> {
        let id = format!("{ns}:{name}");
        let book_name = jstr(v, "name").ok_or_else(|| format!("Book {id}: missing `name`"))?.to_owned();
        if !jbool(v, "use_resource_pack", false) {
            return Err(format!(
                "Book {id} has use_resource_pack set to false. This behaviour was removed in 1.20: the book contents belong in /assets/."
            ));
        }
        if let Some(e) = jstr(v, "extend") {
            return Err(format!("Book {id} is declared to extend {e}. This behaviour was removed in 1.20."));
        }
        let tex = |key: &str, default: &str| qualify("minecraft", jstr(v, key).unwrap_or(default));
        let mut macros: BTreeMap<String, String> = BTreeMap::new();
        // Book.DEFAULT_MACROS
        macros.insert("$(list".into(), "$(li".into());
        macros.insert("/$".into(), "$()".into());
        macros.insert("<br>".into(), "$(br)".into());
        macros.insert("$(item)".into(), "$(#b0b)".into());
        macros.insert("$(thing)".into(), "$(#490)".into());
        if let Some(Value::Object(m)) = v.get("macros") {
            for (k, val) in m {
                macros.insert(k.clone(), as_string(val));
            }
        }
        Ok(Self {
            id,
            ns: ns.to_owned(),
            name: book_name,
            landing_text: jstr(v, "landing_text").unwrap_or("patchouli.gui.lexicon.landing_info").to_owned(),
            book_texture: tex("book_texture", "patchouli:textures/gui/book_brown.png"),
            filler_texture: tex("filler_texture", "patchouli:textures/gui/page_filler.png"),
            crafting_texture: tex("crafting_texture", "patchouli:textures/gui/crafting.png"),
            model: tex("model", "patchouli:book_brown"),
            use_blocky_font: jbool(v, "use_blocky_font", false),
            text_color: color_of(v, "text_color", "000000"),
            header_color: color_of(v, "header_color", "333333"),
            nameplate_color: color_of(v, "nameplate_color", "FFDD00"),
            link_color: color_of(v, "link_color", "0000EE"),
            link_hover_color: color_of(v, "link_hover_color", "8800EE"),
            progress_bar_color: color_of(v, "progress_bar_color", "FFFF55"),
            progress_bar_background: color_of(v, "progress_bar_background", "DDDDDD"),
            open_sound: tex("open_sound", "patchouli:book_open"),
            flip_sound: tex("flip_sound", "patchouli:book_flip"),
            show_progress: jbool(v, "show_progress", true),
            index_icon: jstr(v, "index_icon").unwrap_or("").to_owned(),
            version: match v.get("version") {
                Some(x) => as_string(x),
                None => "0".to_owned(),
            },
            subtitle: jstr(v, "subtitle").unwrap_or("").to_owned(),
            creative_tab: jstr(v, "creative_tab").map(str::to_owned),
            advancements_tab: jstr(v, "advancements_tab").map(str::to_owned),
            no_book: jbool(v, "dont_generate_book", false),
            custom_book_item: jstr(v, "custom_book_item").unwrap_or("").to_owned(),
            show_toasts: jbool(v, "show_toasts", true),
            pause_game: jbool(v, "pause_game", false),
            is_pamphlet: jbool(v, "pamphlet", false),
            i18n: jbool(v, "i18n", false),
            overflow: jstr(v, "text_overflow_mode").and_then(Overflow::parse),
            macros,
        })
    }

    /// `Book.advancementsEnabled`.
    pub fn advancements_enabled(&self) -> bool {
        !config::DISABLE_ADVANCEMENT_LOCKING && !config::NO_ADVANCEMENT_BOOKS.contains(&self.id.as_str())
    }

    /// `Book.getSubtitle`: "<ordinal> Edition", the writer's edition, or the book's own subtitle.
    pub fn subtitle_text(&self, env: &dyn Env) -> String {
        let tr = |key: &str, fallback: &str| env.translate(key).unwrap_or_else(|| fallback.to_owned());
        let edition = match self.version.trim().parse::<i64>() {
            Ok(0) => return env.translate(&self.subtitle).unwrap_or_else(|| self.subtitle.clone()),
            Ok(n) => ordinal(n),
            Err(_) => tr("patchouli.gui.lexicon.dev_edition", "Writer's"),
        };
        tr("patchouli.gui.lexicon.edition_str", "%s Edition").replace("%s", &edition)
    }

    /// The name of the book as its item shows it (`ItemModBook.getName`): the book's name translated.
    pub fn title(&self, env: &dyn Env) -> String {
        env.translate(&self.name).unwrap_or_else(|| self.name.clone())
    }

    /// The item of a mod that has its own book item (`dont_generate_book`).
    pub fn custom_item(&self) -> Option<Stack> {
        if self.no_book { parse_stack(&self.custom_book_item).ok() } else { None }
    }
}

fn ordinal(n: i64) -> String {
    const SUFFIX: [&str; 10] = ["th", "st", "nd", "rd", "th", "th", "th", "th", "th", "th"];
    if matches!(n % 100, 11..=13) { format!("{n}th") } else { format!("{n}{}", SUFFIX[(n % 10).unsigned_abs() as usize]) }
}

/// A page of an entry (`BookPage`): its type and its JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// The place in the `pages` array of the entry (`pageNum`), also of the pages that a `flag` removed.
    pub index: usize,
    /// `namespace:type`; a bare type of the JSON is `patchouli:<type>` when it is one of the built-in pages.
    pub kind: String,
    pub json: Value,
    pub anchor: Option<String>,
    pub advancement: Option<String>,
}

pub const BUILTIN_PAGES: [&str; 16] = [
    "text", "crafting", "smelting", "blasting", "smoking", "campfire", "smithing", "stonecutting", "image", "spotlight",
    "empty", "multiblock", "link", "relations", "entity", "quest",
];

impl Page {
    /// `LexiconPageAdapter.deserialize`: a string is a text page.
    pub fn from_json(v: &Value) -> Page {
        if let Value::String(s) = v {
            let mut o = Map::new();
            o.insert("type".into(), Value::String("patchouli:text".into()));
            o.insert("text".into(), Value::String(s.clone()));
            return Page { index: 0, kind: "patchouli:text".into(), json: Value::Object(o), anchor: None, advancement: None };
        }
        let raw = jstr(v, "type").unwrap_or("patchouli:text").to_owned();
        let kind = if raw.contains(':') {
            raw.clone()
        } else if BUILTIN_PAGES.contains(&raw.as_str()) {
            format!("patchouli:{raw}")
        } else {
            raw.clone()
        };
        Page {
            index: 0,
            kind,
            json: v.clone(),
            anchor: jstr(v, "anchor").map(str::to_owned),
            advancement: jstr(v, "advancement").filter(|s| !s.is_empty()).map(str::to_owned),
        }
    }
    pub fn is_builtin(&self) -> bool {
        self.kind.strip_prefix("patchouli:").is_some_and(|k| BUILTIN_PAGES.contains(&k))
    }
    pub fn flag(&self) -> &str {
        jstr(&self.json, "flag").unwrap_or("")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub parent: Option<String>,
    pub flag: String,
    pub sortnum: i32,
    pub secret: bool,
    pub entries: Vec<String>,
    pub children: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub icon: String,
    pub flag: String,
    pub priority: bool,
    pub secret: bool,
    pub read_by_default: bool,
    pub advancement: Option<String>,
    pub turnin: Option<String>,
    pub sortnum: i32,
    pub entry_color: Option<u32>,
    /// The pages that were added (`BookPage.canAdd`: their `flag`).
    pub pages: Vec<Page>,
    /// The stacks the entry explains, for the search (`relevantStacks`).
    pub relevant: Vec<Stack>,
}
impl Entry {
    /// `BookEntry.getPageFromAnchor` over the visible pages.
    pub fn page_from_anchor(&self, anchor: &str) -> Option<usize> {
        self.pages.iter().position(|p| p.anchor.as_deref() == Some(anchor))
    }
}

/// `EntryDisplayState`; the order is the one of the enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReadState {
    Unread,
    Pending,
    Neutral,
    Completed,
}
impl ReadState {
    pub fn has_icon(self) -> bool {
        self != ReadState::Neutral
    }
    pub fn has_animation(self) -> bool {
        matches!(self, ReadState::Unread | ReadState::Pending)
    }
    /// The `u` of the marking in the book texture.
    pub fn u(self) -> i32 {
        match self {
            ReadState::Unread => 140,
            ReadState::Pending => 148,
            ReadState::Neutral => 0,
            ReadState::Completed => 156,
        }
    }
}

/// The reading state of one book (`PersistentData.BookData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookData {
    pub viewed: Vec<String>,
    /// (entry, spread)
    pub bookmarks: Vec<(String, usize)>,
    /// Newest first; not saved (the original keeps no history between runs).
    pub history: Vec<String>,
    pub completed_quests: Vec<String>,
}

/// `patchouli_data.json`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Persist {
    pub book_gui_scale: i64,
    pub clicked_visualize: bool,
    pub books: BTreeMap<String, BookData>,
    /// Something changed since the last save.
    pub dirty: bool,
}
impl Persist {
    pub fn load(text: &str) -> Self {
        let Ok(v) = parse_json(text) else { return Self::default() };
        let mut p = Persist {
            book_gui_scale: jint(&v, "bookGuiScale", 0),
            clicked_visualize: jbool(&v, "clickedVisualize", false),
            ..Default::default()
        };
        if let Some(Value::Object(books)) = v.get("bookData") {
            for (id, b) in books {
                let strs = |k: &str| -> Vec<String> {
                    b.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default()
                };
                let bookmarks = b
                    .get("bookmarks")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|m| Some((jstr(m, "entry")?.to_owned(), jint(m, "page", 0).max(0) as usize)))
                            .collect()
                    })
                    .unwrap_or_default();
                p.books.insert(
                    id.clone(),
                    BookData {
                        viewed: strs("viewedEntries"),
                        bookmarks,
                        history: strs("history"),
                        completed_quests: strs("completedManualQuests"),
                    },
                );
            }
        }
        p
    }
    pub fn save(&self) -> String {
        let mut books = Map::new();
        for (id, b) in &self.books {
            let mut o = Map::new();
            o.insert("viewedEntries".into(), Value::from(b.viewed.clone()));
            o.insert(
                "bookmarks".into(),
                Value::Array(
                    b.bookmarks
                        .iter()
                        .map(|(e, s)| {
                            let mut m = Map::new();
                            m.insert("entry".into(), Value::String(e.clone()));
                            m.insert("page".into(), Value::from(*s as i64));
                            Value::Object(m)
                        })
                        .collect(),
                ),
            );
            o.insert("completedManualQuests".into(), Value::from(b.completed_quests.clone()));
            books.insert(id.clone(), Value::Object(o));
        }
        let mut root = Map::new();
        root.insert("bookGuiScale".into(), Value::from(self.book_gui_scale));
        root.insert("clickedVisualize".into(), Value::Bool(self.clicked_visualize));
        root.insert("bookData".into(), Value::Object(books));
        serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default()
    }
    pub fn data(&mut self, book: &str) -> &mut BookData {
        self.books.entry(book.to_owned()).or_default()
    }
    pub fn get(&self, book: &str) -> Option<&BookData> {
        self.books.get(book)
    }
}

/// A book with what it holds (`BookContents`).
#[derive(Clone, Debug, Default)]
pub struct Contents {
    pub categories: BTreeMap<String, Category>,
    pub entries: BTreeMap<String, Entry>,
    /// Template files by id (`ns:path`), as JSON.
    pub templates: HashMap<String, Value>,
    /// Item id to (entry id, spread): where an item is explained (`recipeMappings`).
    pub recipe_map: HashMap<String, (String, usize)>,
    pub pamphlet: Option<String>,
    /// Files that could not be read or used.
    pub errors: Vec<String>,
}

/// A book that is loaded.
#[derive(Clone, Debug)]
pub struct Book {
    pub info: BookInfo,
    pub contents: Contents,
    /// The language the contents were loaded in.
    pub lang: String,
}

/// `PatchouliConfig.getConfigFlag`.
pub fn config_flag(env: &dyn Env, name: &str) -> bool {
    if name.starts_with('&') {
        return name.replace(['&', '|'], "").split(',').all(|t| config_flag(env, t));
    }
    if name.starts_with('|') {
        return name.replace(['&', '|'], "").split(',').any(|t| config_flag(env, t));
    }
    let (target, name) = match name.strip_prefix('!') {
        Some(rest) => (false, rest),
        None => (true, name),
    };
    let name = name.trim().to_lowercase();
    let value = if let Some(ns) = name.strip_prefix("mod:") {
        env.mod_loaded(ns)
    } else {
        match name.as_str() {
            "debug" => false,
            "advancements_disabled" => config::DISABLE_ADVANCEMENT_LOCKING,
            "testing_mode" => config::TESTING_MODE,
            other => other
                .strip_prefix("advancements_disabled_")
                .is_some_and(|b| config::NO_ADVANCEMENT_BOOKS.iter().any(|x| x.to_lowercase() == b)),
        }
    };
    value == target
}

impl Contents {
    /// `BookEntry.isLocked`.
    pub fn entry_locked(&self, env: &dyn Env, book: &BookInfo, id: &str) -> bool {
        let Some(e) = self.entries.get(id) else { return false };
        let locked = e.advancement.as_deref().is_some_and(|a| !env.advancement_done(a));
        if e.secret { locked } else { book.advancements_enabled() && locked }
    }
    pub fn entry_hidden(&self, env: &dyn Env, book: &BookInfo, id: &str) -> bool {
        self.entries.get(id).is_some_and(|e| e.secret) && self.entry_locked(env, book, id)
    }
    /// `BookCategory.isLocked` (`updateLockStatus`): locked while nothing in it is unlocked; an empty one is open.
    pub fn category_locked(&self, env: &dyn Env, book: &BookInfo, id: &str) -> bool {
        self.category_locked_in(env, book, id, 0)
    }
    fn category_locked_in(&self, env: &dyn Env, book: &BookInfo, id: &str, depth: usize) -> bool {
        if !book.advancements_enabled() || depth > 16 {
            return false;
        }
        let Some(c) = self.categories.get(id) else { return false };
        if c.children.is_empty() && c.entries.is_empty() {
            return false;
        }
        if c.children.iter().any(|ch| !self.category_locked_in(env, book, ch, depth + 1)) {
            return false;
        }
        !c.entries.iter().any(|e| !self.entry_locked(env, book, e))
    }
    pub fn category_hidden(&self, env: &dyn Env, book: &BookInfo, id: &str) -> bool {
        self.categories.get(id).is_some_and(|c| c.secret) && self.category_locked(env, book, id)
    }
    /// `PageQuest.isCompleted`.
    pub fn quest_completed(&self, env: &dyn Env, persist: &Persist, book: &BookInfo, entry: &str, page: &Page) -> bool {
        match jstr(&page.json, "trigger") {
            None => persist.get(&book.id).is_some_and(|d| d.completed_quests.iter().any(|q| q == entry)),
            Some(t) => env.advancement_done(t),
        }
    }
    /// `BookEntry.computeReadState`.
    pub fn entry_state(&self, env: &dyn Env, persist: &Persist, book: &BookInfo, id: &str) -> ReadState {
        let Some(e) = self.entries.get(id) else { return ReadState::Neutral };
        let viewed = persist.get(&book.id).is_some_and(|d| d.viewed.iter().any(|v| v == id));
        if !e.read_by_default && !self.entry_locked(env, book, id) && !viewed {
            return ReadState::Unread;
        }
        if e.turnin.as_deref().is_some_and(|t| !env.advancement_done(t)) {
            return ReadState::Pending;
        }
        if e.pages.iter().any(|p| p.kind == "patchouli:quest" && self.quest_completed(env, persist, book, id, p)) {
            return ReadState::Completed;
        }
        ReadState::Neutral
    }
    /// `BookCategory.computeReadState`.
    pub fn category_state(&self, env: &dyn Env, persist: &Persist, book: &BookInfo, id: &str) -> ReadState {
        self.category_state_in(env, persist, book, id, 0)
    }
    fn category_state_in(&self, env: &dyn Env, persist: &Persist, book: &BookInfo, id: &str, depth: usize) -> ReadState {
        let Some(c) = self.categories.get(id) else { return ReadState::Neutral };
        if depth > 16 {
            return ReadState::Neutral;
        }
        let mut best = ReadState::Neutral;
        for e in &c.entries {
            if !self.entry_locked(env, book, e) {
                best = best.min(self.entry_state(env, persist, book, e));
            }
        }
        for ch in &c.children {
            best = best.min(self.category_state_in(env, persist, book, ch, depth + 1));
        }
        best
    }
    /// `BookEntry.compareTo`.
    pub fn compare_entries(&self, env: &dyn Env, persist: &Persist, book: &BookInfo, a: &str, b: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let (Some(ea), Some(eb)) = (self.entries.get(a), self.entries.get(b)) else { return Ordering::Equal };
        let (la, lb) = (self.entry_locked(env, book, a), self.entry_locked(env, book, b));
        if la != lb {
            return if la { Ordering::Greater } else { Ordering::Less };
        }
        let (sa, sb) = (self.entry_state(env, persist, book, a), self.entry_state(env, persist, book, b));
        if sa != sb {
            return sa.cmp(&sb);
        }
        if ea.priority != eb.priority {
            return if ea.priority { Ordering::Less } else { Ordering::Greater };
        }
        match ea.sortnum.cmp(&eb.sortnum) {
            Ordering::Equal => display_name(env, book, &ea.name).cmp(&display_name(env, book, &eb.name)),
            other => other,
        }
    }
    /// `BookCategory.compareTo`.
    pub fn compare_categories(&self, env: &dyn Env, book: &BookInfo, a: &str, b: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let (Some(ca), Some(cb)) = (self.categories.get(a), self.categories.get(b)) else { return Ordering::Equal };
        if book.advancements_enabled() {
            let (la, lb) = (self.category_locked(env, book, a), self.category_locked(env, book, b));
            if la != lb {
                return if la { Ordering::Greater } else { Ordering::Less };
            }
        }
        match ca.sortnum.cmp(&cb.sortnum) {
            Ordering::Equal => display_name(env, book, &ca.name).cmp(&display_name(env, book, &cb.name)),
            other => other,
        }
    }
    /// `BookContents.getEntryForStack`.
    pub fn entry_for_stack(&self, item: &str) -> Option<&(String, usize)> {
        self.recipe_map.get(item)
    }
    /// The root categories, in the order the landing page lists them.
    pub fn root_categories(&self, env: &dyn Env, book: &BookInfo) -> Vec<String> {
        let mut v: Vec<String> = self
            .categories
            .values()
            .filter(|c| c.parent.is_none() && !self.category_hidden(env, book, &c.id))
            .map(|c| c.id.clone())
            .collect();
        v.sort_by(|a, b| self.compare_categories(env, book, a, b));
        v
    }
}

/// `BookEntry.getName().getString()`.
pub fn display_name(env: &dyn Env, book: &BookInfo, name: &str) -> String {
    if book.i18n { env.translate(name).unwrap_or_else(|| name.to_owned()) } else { name.to_owned() }
}

// ------------------------------------------------------------------------------------------------
// Loading
// ------------------------------------------------------------------------------------------------

/// A book found in the data (`data/<ns>/patchouli_books/<name>/book.json`).
#[derive(Clone, Debug, PartialEq)]
pub struct BookRef {
    pub ns: String,
    pub name: String,
}
impl BookRef {
    pub fn id(&self) -> String {
        format!("{}:{}", self.ns, self.name)
    }
}

/// `BookRegistry.init`: every `book.json` of every namespace. A book that cannot be read comes back with its error.
pub fn discover(files: &dyn Files) -> Vec<(BookRef, Result<BookInfo, String>)> {
    let mut out = Vec::new();
    for ns in files.list("data") {
        let dir = format!("data/{ns}/patchouli_books");
        for name in files.list(&dir) {
            let Some(text) = files.read(&format!("{dir}/{name}/book.json")) else { continue };
            let info = parse_json(&text).map_err(|e| format!("{ns}:{name}: {e}")).and_then(|v| BookInfo::from_json(&ns, &name, &v));
            out.push((BookRef { ns: ns.clone(), name }, info));
        }
    }
    out
}

/// `BookContentsBuilder.loadAndBuildFor`. Unlike the original, a file that does not load is left out and listed in
/// `errors`; the rest of the book is kept.
pub fn load_book(files: &dyn Files, env: &dyn Env, info: BookInfo) -> Book {
    let lang = env.lang_code();
    let contents = load_contents(files, env, &info, &lang);
    Book { info, contents, lang }
}

fn load_contents(files: &dyn Files, env: &dyn Env, info: &BookInfo, lang: &str) -> Contents {
    let name = info.id.split_once(':').map_or("", |(_, n)| n);
    let root = format!("assets/{}/patchouli_books/{name}", info.ns);
    let mut c = Contents::default();
    let localized = |kind: &str, rel: &str| -> Option<(String, String)> {
        let primary = format!("{root}/{lang}/{kind}/{rel}");
        if let Some(t) = files.read(&primary) {
            return Some((primary, t));
        }
        let fallback = format!("{root}/{DEFAULT_LANG}/{kind}/{rel}");
        files.read(&fallback).map(|t| (fallback, t))
    };
    let stem = |rel: &str| rel.strip_suffix(".json").unwrap_or(rel).to_owned();
    // categories
    for rel in walk(files, &format!("{root}/{DEFAULT_LANG}/categories")) {
        let id = format!("{}:{}", info.ns, stem(&rel));
        let Some((file, text)) = localized("categories", &rel) else { continue };
        match parse_json(&text) {
            Ok(v) if v.is_object() => {
                let flag = jstr(&v, "flag").unwrap_or("").to_owned();
                if !flag.is_empty() && !config_flag(env, &flag) {
                    continue;
                }
                let (Some(cname), Some(desc), Some(icon)) = (jstr(&v, "name"), jstr(&v, "description"), jstr(&v, "icon")) else {
                    c.errors.push(format!("{file}: a category needs name, description and icon"));
                    continue;
                };
                c.categories.insert(
                    id.clone(),
                    Category {
                        id,
                        name: cname.to_owned(),
                        description: desc.to_owned(),
                        icon: icon.to_owned(),
                        parent: jstr(&v, "parent").filter(|p| !p.is_empty()).map(str::to_owned),
                        flag,
                        sortnum: jint(&v, "sortnum", 0) as i32,
                        secret: jbool(&v, "secret", false),
                        entries: Vec::new(),
                        children: Vec::new(),
                    },
                );
            }
            Ok(_) => c.errors.push(format!("{file}: not an object")),
            Err(e) => c.errors.push(format!("{file}: {e}")),
        }
    }
    // templates
    for rel in walk(files, &format!("{root}/{DEFAULT_LANG}/templates")) {
        let id = format!("{}:{}", info.ns, stem(&rel));
        let Some((file, text)) = localized("templates", &rel) else { continue };
        match parse_json(&text) {
            Ok(v) if v.is_object() => {
                c.templates.insert(id, v);
            }
            Ok(_) => c.errors.push(format!("{file}: not an object")),
            Err(e) => c.errors.push(format!("{file}: {e}")),
        }
    }
    // entries
    let mut raw: Vec<(Entry, Vec<Value>, Option<Value>)> = Vec::new();
    for rel in walk(files, &format!("{root}/{DEFAULT_LANG}/entries")) {
        let id = format!("{}:{}", info.ns, stem(&rel));
        let Some((file, text)) = localized("entries", &rel) else { continue };
        let v = match parse_json(&text) {
            Ok(v) if v.is_object() => v,
            Ok(_) => {
                c.errors.push(format!("{file}: not an object"));
                continue;
            }
            Err(e) => {
                c.errors.push(format!("{file}: {e}"));
                continue;
            }
        };
        let flag = jstr(&v, "flag").unwrap_or("").to_owned();
        if !flag.is_empty() && !config_flag(env, &flag) {
            continue;
        }
        let (Some(ename), Some(category), Some(icon)) = (jstr(&v, "name"), jstr(&v, "category"), jstr(&v, "icon")) else {
            c.errors.push(format!("{file}: an entry needs name, category and icon"));
            continue;
        };
        if !category.contains(':') {
            c.errors.push(format!("{file}: `category` must be fully qualified (domain:name). Hint: Try `{}:{category}`", info.ns));
            continue;
        }
        let pages_json = v.get("pages").and_then(Value::as_array).cloned().unwrap_or_default();
        let entry = Entry {
            id: id.clone(),
            name: ename.to_owned(),
            category: category.to_owned(),
            icon: icon.to_owned(),
            flag,
            priority: jbool(&v, "priority", false),
            secret: jbool(&v, "secret", false),
            read_by_default: jbool(&v, "read_by_default", false),
            advancement: jstr(&v, "advancement").filter(|s| !s.is_empty()).map(|s| qualify("minecraft", s)),
            turnin: jstr(&v, "turnin").filter(|s| !s.is_empty()).map(|s| qualify("minecraft", s)),
            sortnum: jint(&v, "sortnum", 0) as i32,
            entry_color: jstr(&v, "entry_color").and_then(|s| u32::from_str_radix(s.trim(), 16).ok()),
            pages: pages_json
                .iter()
                .enumerate()
                .map(|(i, p)| Page { index: i, ..Page::from_json(p) })
                .filter(|p| p.flag().is_empty() || config_flag(env, p.flag()))
                .collect(),
            relevant: Vec::new(),
        };
        raw.push((entry, pages_json, v.get("extra_recipe_mappings").cloned()));
    }
    // links between categories and entries
    let cat_ids: Vec<String> = c.categories.keys().cloned().collect();
    for id in &cat_ids {
        let parent = c.categories[id].parent.clone();
        if let Some(p) = parent {
            let p = if p.contains(':') { p } else { format!("{}:{p}", info.ns) };
            if c.categories.contains_key(&p) {
                c.categories.get_mut(id).unwrap().parent = Some(p.clone());
                c.categories.get_mut(&p).unwrap().children.push(id.clone());
            } else {
                c.errors.push(format!("Category {id} specifies parent {p}, but it could not be found"));
                c.categories.get_mut(id).unwrap().parent = None;
            }
        }
    }
    let mut raw_pages: HashMap<String, (Vec<Value>, Option<Value>)> = HashMap::new();
    for (entry, all, extra) in raw {
        if !c.categories.contains_key(&entry.category) {
            c.errors.push(format!("Entry {} does not have a valid category ({}).", entry.id, entry.category));
            continue;
        }
        let id = entry.id.clone();
        c.categories.get_mut(&entry.category).unwrap().entries.push(id.clone());
        raw_pages.insert(id.clone(), (all, extra));
        c.entries.insert(id, entry);
    }
    // the stacks each entry explains
    let ids: Vec<String> = c.entries.keys().cloned().collect();
    for id in &ids {
        let Some((all, extra)) = raw_pages.get(id) else { continue };
        let mut found: Vec<(Stack, usize)> = Vec::new();
        for (i, pj) in all.iter().enumerate() {
            let page = Page::from_json(pj);
            if !page.flag().is_empty() && !config_flag(env, page.flag()) {
                continue;
            }
            for s in relevant_stacks(env, &c, info, &page) {
                found.push((s, i));
            }
        }
        if let Some(Value::Object(m)) = extra {
            for (key, page_no) in m {
                let n = page_no.as_u64().unwrap_or(0) as usize;
                match parse_stack_list(env, key) {
                    Ok(stacks) if !stacks.is_empty() && n < all.len() => {
                        for s in stacks {
                            found.push((s, n));
                        }
                    }
                    _ => c.errors.push(format!("Invalid extra recipe mapping: {key} to page {n} in entry {id}")),
                }
            }
        }
        for (s, page) in found {
            if s.is_empty() {
                continue;
            }
            c.recipe_map.insert(s.id.clone(), (id.clone(), page / 2));
            c.entries.get_mut(id).unwrap().relevant.push(s);
        }
    }
    if info.is_pamphlet {
        if c.categories.len() == 1 {
            c.pamphlet = c.categories.keys().next().cloned();
        } else {
            c.errors.push(format!("Pamphlet {} should have exactly one category but instead there were {}", info.id, c.categories.len()));
        }
    }
    c
}

// ------------------------------------------------------------------------------------------------
// Recipes (the crafting, cooking, stonecutting and smithing pages)
// ------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeKind {
    Crafting,
    Smelting,
    Blasting,
    Smoking,
    Campfire,
    Stonecutting,
    Smithing,
}
impl RecipeKind {
    /// The recipe pages by type.
    pub fn of_page(kind: &str) -> Option<Self> {
        Some(match kind.strip_prefix("patchouli:")? {
            "crafting" => Self::Crafting,
            "smelting" => Self::Smelting,
            "blasting" => Self::Blasting,
            "smoking" => Self::Smoking,
            "campfire" => Self::Campfire,
            "stonecutting" => Self::Stonecutting,
            "smithing" => Self::Smithing,
            _ => return None,
        })
    }
    fn matches(self, recipe_type: &str) -> bool {
        let t = recipe_type.strip_prefix("minecraft:").unwrap_or(recipe_type);
        match self {
            Self::Crafting => matches!(t, "crafting_shaped" | "crafting_shapeless"),
            Self::Smelting => t == "smelting",
            Self::Blasting => t == "blasting",
            Self::Smoking => t == "smoking",
            Self::Campfire => t == "campfire_cooking",
            Self::Stonecutting => t == "stonecutting",
            Self::Smithing => matches!(t, "smithing_transform" | "smithing_trim"),
        }
    }
    /// `Recipe.getToastSymbol`: the block that makes it.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Crafting => "minecraft:crafting_table",
            Self::Smelting => "minecraft:furnace",
            Self::Blasting => "minecraft:blast_furnace",
            Self::Smoking => "minecraft:smoker",
            Self::Campfire => "minecraft:campfire",
            Self::Stonecutting => "minecraft:stonecutter",
            Self::Smithing => "minecraft:smithing_table",
        }
    }
    /// The height of one recipe on the page (`getRecipeHeight`).
    pub fn height(self) -> i32 {
        match self {
            Self::Crafting => 78,
            Self::Smithing => 60,
            _ => 45,
        }
    }
}

/// A recipe as a page draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct RecipeView {
    pub kind: RecipeKind,
    pub shaped: bool,
    /// The grid width of a shaped recipe; 3 otherwise.
    pub width: usize,
    /// The ingredients; each is the stacks that fit (empty for a blank cell). Smithing: base, addition, template.
    pub ingredients: Vec<Vec<Stack>>,
    pub result: Stack,
}

fn ingredient_stacks(env: &dyn Env, v: &Value) -> Vec<Stack> {
    match v {
        Value::String(s) => match s.strip_prefix('#') {
            Some(tag) => env.tag_items(&qualify("minecraft", tag)).into_iter().map(|i| Stack::new(&i, 1)).collect(),
            None => vec![Stack::new(&qualify("minecraft", s), 1)],
        },
        Value::Array(a) => {
            let mut out: Vec<Stack> = Vec::new();
            for x in a {
                for s in ingredient_stacks(env, x) {
                    if !out.iter().any(|o| o.id == s.id) {
                        out.push(s);
                    }
                }
            }
            out
        }
        Value::Object(o) => {
            if let Some(i) = o.get("item").and_then(Value::as_str) {
                vec![Stack::new(&qualify("minecraft", i), 1)]
            } else if let Some(t) = o.get("tag").and_then(Value::as_str) {
                env.tag_items(&qualify("minecraft", t)).into_iter().map(|i| Stack::new(&i, 1)).collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

fn result_stack(v: &Value) -> Option<Stack> {
    match v {
        Value::String(s) => Some(Stack::new(&qualify("minecraft", s), 1)),
        Value::Object(o) => {
            let id = o.get("id").or_else(|| o.get("item")).and_then(Value::as_str)?;
            Some(Stack::new(&qualify("minecraft", id), o.get("count").and_then(Value::as_u64).unwrap_or(1) as u32))
        }
        _ => None,
    }
}

/// `ShapedRecipePattern.shrink`: the pattern without its empty border rows and columns.
fn shrink(pattern: &[Vec<char>]) -> Vec<Vec<char>> {
    let blank = |r: &Vec<char>| r.iter().all(|c| *c == ' ');
    let mut rows: Vec<Vec<char>> = pattern.iter().skip_while(|r| blank(r)).cloned().collect();
    while rows.last().is_some_and(blank) {
        rows.pop();
    }
    let left = rows.iter().filter_map(|r| r.iter().position(|c| *c != ' ')).min();
    let right = rows.iter().filter_map(|r| r.iter().rposition(|c| *c != ' ')).max();
    match (left, right) {
        (Some(l), Some(r)) => rows.iter().map(|row| row.iter().skip(l).take(r + 1 - l).copied().collect()).collect(),
        _ => Vec::new(),
    }
}

/// `PageDoubleRecipeRegistry.loadRecipe`: the recipe `id` if it is of the page's kind.
pub fn load_recipe(env: &dyn Env, kind: RecipeKind, id: &str) -> Option<RecipeView> {
    let json = env.recipe(&qualify("minecraft", id))?;
    let rtype = jstr(&json, "type")?;
    if !kind.matches(rtype) {
        return None;
    }
    let t = rtype.strip_prefix("minecraft:").unwrap_or(rtype);
    match kind {
        RecipeKind::Crafting => {
            let result = result_stack(json.get("result")?)?;
            if t == "crafting_shaped" {
                let pattern: Vec<Vec<char>> =
                    json.get("pattern")?.as_array()?.iter().filter_map(|r| r.as_str().map(|s| s.chars().collect())).collect();
                let pattern = shrink(&pattern);
                let width = pattern.first().map_or(0, Vec::len);
                let key = json.get("key")?.as_object()?;
                let mut ingredients = Vec::new();
                for row in &pattern {
                    for c in row {
                        ingredients.push(if *c == ' ' { Vec::new() } else { ingredient_stacks(env, key.get(&c.to_string())?) });
                    }
                }
                Some(RecipeView { kind, shaped: true, width: width.max(1), ingredients, result })
            } else {
                let ingredients = json.get("ingredients")?.as_array()?.iter().map(|i| ingredient_stacks(env, i)).collect();
                Some(RecipeView { kind, shaped: false, width: 3, ingredients, result })
            }
        }
        RecipeKind::Smithing => {
            let result = result_stack(json.get("result").unwrap_or(&Value::Null)).unwrap_or_default();
            let ing = |k: &str| json.get(k).map(|v| ingredient_stacks(env, v)).unwrap_or_default();
            Some(RecipeView { kind, shaped: true, width: 3, ingredients: vec![ing("base"), ing("addition"), ing("template")], result })
        }
        _ => {
            let result = result_stack(json.get("result")?)?;
            let ingredient = json.get("ingredient").map(|v| ingredient_stacks(env, v)).unwrap_or_default();
            Some(RecipeView { kind, shaped: true, width: 3, ingredients: vec![ingredient], result })
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Templates (BookTemplate, TemplateComponent, TemplateInclusion, VariableAssigner)
// ------------------------------------------------------------------------------------------------

/// A resolved component of a template.
#[derive(Clone, Debug, PartialEq)]
pub struct Comp {
    pub x: i32,
    pub y: i32,
    pub group: String,
    pub flag: String,
    pub advancement: String,
    pub negate_advancement: bool,
    pub guard_pass: bool,
    pub kind: CompKind,
}
#[derive(Clone, Debug, PartialEq)]
pub enum CompKind {
    Text { text: String, color: Option<u32>, max_width: i32, line_height: i32 },
    Item { items: Vec<Stack>, framed: bool, link_recipe: bool },
    Image { image: String, u: i32, v: i32, width: i32, height: i32, tw: i32, th: i32, scale: f32 },
    Header { text: String, color: Option<u32>, centered: bool, scale: f32 },
    Separator,
    Frame,
    Entity { id: String, render_size: f32, rotate: bool, default_rotation: f32 },
    Tooltip { lines: Vec<String>, width: i32, height: i32 },
    /// `patchouli:custom` (Java code) and types nobody registered: nothing to draw.
    Unknown(String),
}
impl Comp {
    /// `TemplateComponent.getVisibleStatus`.
    pub fn visible(&self, env: &dyn Env) -> bool {
        if !self.guard_pass {
            return false;
        }
        if !self.flag.is_empty() && !config_flag(env, &self.flag) {
            return false;
        }
        if !self.advancement.is_empty() {
            return env.advancement_done(&self.advancement) != self.negate_advancement;
        }
        true
    }
}

/// `TemplateInclusion`.
#[derive(Clone, Debug)]
struct Inclusion {
    template: String,
    as_name: String,
    bindings: Map<String, Value>,
    x: i32,
    y: i32,
    visited: Vec<String>,
}
impl Inclusion {
    fn from_json(v: &Value) -> Result<Self, String> {
        let template = jstr(v, "template").unwrap_or("").to_owned();
        let as_name = jstr(v, "as").unwrap_or("").to_owned();
        if template.is_empty() || as_name.is_empty() {
            return Err("Template inclusion must define both \"template\" and \"as\" fields.".to_owned());
        }
        Ok(Self {
            template,
            as_name,
            bindings: v.get("using").and_then(Value::as_object).cloned().unwrap_or_default(),
            x: jint(v, "x", 0) as i32,
            y: jint(v, "y", 0) as i32,
            visited: Vec::new(),
        })
    }
    fn upper_merge(&mut self, parent: Option<&Inclusion>) -> Result<(), String> {
        let Some(parent) = parent else { return Ok(()) };
        if parent.visited.contains(&self.template) {
            return Err(format!("Breaking when include template {}, circular dependencies aren't allowed (stack = {:?})", self.template, parent.visited));
        }
        self.visited = parent.visited.clone();
        self.visited.push(self.template.clone());
        self.as_name = parent.qualify_name(&self.as_name);
        self.x += parent.x;
        self.y += parent.y;
        let keys: Vec<String> = self.bindings.keys().cloned().collect();
        for k in keys {
            if let Some(real) = self.bindings[&k].as_str().and_then(|s| s.strip_prefix('#')) {
                if let Some(v) = parent.bindings.get(real) {
                    self.bindings.insert(k, v.clone());
                }
            }
        }
        Ok(())
    }
    fn qualify_name(&self, name: &str) -> String {
        let prefixed = name.starts_with('#');
        let query = name.strip_prefix('#').unwrap_or(name);
        let result = as_string(self.bindings.get(query).unwrap_or(&Value::Null));
        if let Some(up) = result.strip_prefix('#') {
            return up.to_owned();
        }
        format!("{}{}{}", if prefixed { "#" } else { "" }, self.as_name, if query.is_empty() { String::new() } else { format!(".{query}") })
    }
    fn attempt_lookup(&self, key: &str) -> Option<Value> {
        let key = key.strip_prefix('#').unwrap_or(key);
        let v = self.bindings.get(key)?;
        let s = as_string(v);
        if s.is_empty() || v.as_str().is_some_and(|x| x.starts_with('#')) { None } else { Some(v.clone()) }
    }
}

/// `IVariableProvider`: the variables a template reads (the JSON of the page, or what an inclusion makes of it).
pub trait Vars {
    fn has(&self, key: &str) -> bool;
    fn get(&self, key: &str) -> Value;
}
impl Vars for Value {
    fn has(&self, key: &str) -> bool {
        Value::get(self, key).is_some()
    }
    fn get(&self, key: &str) -> Value {
        Value::get(self, key).cloned().unwrap_or(Value::Null)
    }
}
/// `TemplateInclusion.wrapProvider`.
struct WrappedVars<'a> {
    inc: &'a Inclusion,
    base: &'a dyn Vars,
}
impl Vars for WrappedVars<'_> {
    fn has(&self, key: &str) -> bool {
        self.inc.attempt_lookup(key).is_some() || self.base.has(&self.inc.qualify_name(key))
    }
    fn get(&self, key: &str) -> Value {
        self.inc.attempt_lookup(key).unwrap_or_else(|| self.base.get(&self.inc.qualify_name(key)))
    }
}

/// `IComponentProcessor`: the Java class that a template names with `processor` and that fills in its variables
/// (a recipe of the mod, for instance). Native mods register theirs by the name of the Java class
/// (`register_processor`); a template whose class nobody registered gets only what the JSON of its page says.
pub trait Processor {
    /// Called once when the template is compiled, with the variables of the page.
    fn setup(&mut self, _env: &dyn Env, _vars: &dyn Vars) {}
    /// The value of the variable `key` (without its `#`), or `None` to let the page's own variables answer.
    fn process(&self, env: &dyn Env, key: &str) -> Option<Value>;
    /// `allowRender(group)`: a group of components may be hidden.
    fn allow_render(&self, _group: &str) -> bool {
        true
    }
}
pub type ProcessorFactory = fn() -> Box<dyn Processor>;
static PROCESSORS: std::sync::Mutex<BTreeMap<String, ProcessorFactory>> = std::sync::Mutex::new(BTreeMap::new());
/// Registers the processor of a Java class (`wayoftime.bloodmagic.compat.patchouli.processors.ARCProcessor`).
pub fn register_processor(class: &str, factory: ProcessorFactory) {
    if let Ok(mut m) = PROCESSORS.lock() {
        m.insert(class.to_owned(), factory);
    }
}
fn make_processor(class: &str) -> Option<Box<dyn Processor>> {
    let f = PROCESSORS.lock().ok()?.get(class).copied()?;
    Some(f())
}

struct VarCtx<'a> {
    env: &'a dyn Env,
    vars: &'a dyn Vars,
    enc: Option<&'a Inclusion>,
    processor: Option<&'a dyn Processor>,
    cache: HashMap<String, Value>,
}

/// `VariableAssigner`'s lookup of one JSON value: strings are variables or contain some, the rest stays as it is.
fn lookup(cx: &mut VarCtx<'_>, input: &Value) -> Value {
    if let Value::String(s) = input {
        if let Some(v) = resolve_string(cx, s) {
            return v;
        }
    }
    input.clone()
}

fn resolve_string(cx: &mut VarCtx<'_>, curr: &str) -> Option<Value> {
    if curr.is_empty() {
        return None;
    }
    let mut s = curr.to_owned();
    // INLINE_VAR_PATTERN `([^#]*)(#[^#]+)#(.*)`
    for _ in 0..64 {
        let Some(first) = s.find('#') else { break };
        let rest = &s[first + 1..];
        let Some(second_rel) = rest.find('#') else { break };
        if second_rel == 0 {
            break;
        }
        let before = s[..first].to_owned();
        let var = s[first..first + 1 + second_rel].to_owned();
        let after = rest[second_rel + 1..].to_owned();
        if after.contains('\n') {
            break;
        }
        let resolved = as_string(&resolve_functions(cx, &var));
        s = format!("{before}{resolved}{after}");
    }
    Some(resolve_functions(cx, &s))
}

fn resolve_functions(cx: &mut VarCtx<'_>, curr: &str) -> Value {
    if let Some(v) = cx.cache.get(curr) {
        return v.clone();
    }
    // FUNCTION_PATTERN `(.+)->(.+)`: the last arrow splits.
    let out = if let Some(idx) = curr.rfind("->").filter(|&i| i > 0 && i + 2 < curr.len()) {
        let (arg, func) = (&curr[..idx], &curr[idx + 2..]);
        let parsed = resolve_functions(cx, arg);
        apply_function(cx.env, func, &parsed)
    } else {
        resolve_var(cx, curr)
    };
    cx.cache.insert(curr.to_owned(), out.clone());
    out
}

fn resolve_var(cx: &mut VarCtx<'_>, original: &str) -> Value {
    if !original.starts_with('#') {
        return Value::String(original.to_owned());
    }
    let mut curr = original.to_owned();
    if let Some(enc) = cx.enc {
        if let Some(v) = enc.attempt_lookup(&curr) {
            return v;
        }
        curr = enc.qualify_name(&curr);
    }
    let key = curr.strip_prefix('#').unwrap_or(&curr);
    if let Some(p) = cx.processor {
        if let Some(v) = p.process(cx.env, original.strip_prefix('#').unwrap_or(original)) {
            return v;
        }
    }
    if cx.vars.has(key) { cx.vars.get(key) } else { Value::Null }
}

fn capitalize(s: &str, fully: bool) -> String {
    let mut out = String::new();
    let mut start = true;
    for c in s.chars() {
        if c.is_whitespace() {
            start = true;
            out.push(c);
        } else if start {
            out.extend(c.to_uppercase());
            start = false;
        } else if fully {
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn apply_function(env: &dyn Env, func: &str, arg: &Value) -> Value {
    let s = || as_string(arg);
    let one_stack = || -> Stack {
        match arg {
            Value::String(t) => parse_stack(t).unwrap_or_default(),
            other => stacks_of(env, other).into_iter().next().unwrap_or_default(),
        }
    };
    match func {
        "iname" => {
            let st = one_stack();
            Value::String(if st.id.is_empty() { "Air".to_owned() } else { env.item_name(&st) })
        }
        "icount" => Value::from(one_stack().count),
        "ename" => {
            let id = s();
            let id = id.split('{').next().unwrap_or("").to_owned();
            Value::String(env.entity_name(&qualify("minecraft", &id)).unwrap_or(id))
        }
        "lower" => Value::String(s().to_lowercase()),
        "upper" => Value::String(s().to_uppercase()),
        "trim" => Value::String(s().trim().to_owned()),
        "capital" => Value::String(capitalize(&s(), false)),
        "fcapital" => Value::String(capitalize(&s(), true)),
        "i18n" => {
            let key = s();
            Value::String(env.translate(&key).unwrap_or(key))
        }
        "exists" => Value::Bool(!arg.is_null()),
        "iexists" => {
            let st = one_stack();
            Value::Bool(!st.is_empty() && env.item_exists(&st.id))
        }
        "inv" => Value::Bool(!as_bool(arg)),
        "stacks" => Value::Array(
            stacks_of(env, arg)
                .into_iter()
                .map(|st| {
                    let mut m = Map::new();
                    m.insert("item".into(), Value::String(st.id.clone()));
                    if st.count != 1 {
                        m.insert("count".into(), Value::from(st.count));
                    }
                    Value::Object(m)
                })
                .collect(),
        ),
        _ => Value::Null,
    }
}

/// `TextComponentVariableSerializer`: a string, or the text of a JSON component (its styles are dropped).
pub fn component_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(component_text).collect(),
        Value::Object(o) => {
            let mut out = String::new();
            if let Some(t) = o.get("text") {
                out.push_str(&as_string(t));
            } else if let Some(t) = o.get("translate").and_then(Value::as_str) {
                out.push_str(t);
            }
            if let Some(extra) = o.get("extra") {
                out.push_str(&component_text(extra));
            }
            out
        }
        other => as_string(other),
    }
}

fn hex_color(v: &Value) -> Option<u32> {
    u32::from_str_radix(as_string(v).trim(), 16).ok().map(|c| c & 0xFF_FFFF)
}

struct Compiler<'a> {
    env: &'a dyn Env,
    contents: &'a Contents,
    info: &'a BookInfo,
    vars: &'a dyn Vars,
}
impl Compiler<'_> {
    /// `BookTemplate.createTemplate`.
    fn template(&self, kind: &str) -> Result<&Value, String> {
        let key = qualify(&self.info.ns, kind);
        self.contents.templates.get(&key).ok_or_else(|| format!("Template {key} does not exist"))
    }

    /// `BookTemplate.compile`: the components of `tpl` (its own first, then what it includes) with their variables resolved.
    fn compile(&self, tpl: &Value, enc: Option<&Inclusion>) -> Result<Vec<Comp>, String> {
        // createProcessor, then setup with the variables as this template sees them
        let mut processor = jstr(tpl, "processor").filter(|c| !c.is_empty()).and_then(make_processor);
        if let Some(p) = processor.as_mut() {
            match enc {
                Some(e) => p.setup(self.env, &WrappedVars { inc: e, base: self.vars }),
                None => p.setup(self.env, self.vars),
            }
        }
        let processor = processor;
        let mut comps: Vec<Comp> = Vec::new();
        if let Some(Value::Array(cs)) = tpl.get("components") {
            for c in cs {
                if let Some(comp) = self.component(c, enc, processor.as_deref()) {
                    comps.push(comp);
                }
            }
        }
        if let Some(Value::Array(incs)) = tpl.get("include") {
            for inc in incs {
                let mut inc = Inclusion::from_json(inc)?;
                inc.upper_merge(enc)?;
                // TemplateInclusion.process: bindings that name a variable take the processor's answer.
                if let Some(p) = processor.as_deref() {
                    let keys: Vec<String> = inc.bindings.keys().cloned().collect();
                    for k in keys {
                        if let Some(real) = inc.bindings[&k].as_str().and_then(|v| v.strip_prefix('#')) {
                            if let Some(v) = p.process(self.env, real) {
                                inc.bindings.insert(k, v);
                            }
                        }
                    }
                }
                let sub = self.template(&inc.template)?;
                comps.extend(self.compile(sub, Some(&inc))?);
            }
        }
        Ok(comps)
    }

    fn component(&self, c: &Value, enc: Option<&Inclusion>, processor: Option<&dyn Processor>) -> Option<Comp> {
        let ty = jstr(c, "type")?;
        let ty = qualify("minecraft", ty);
        let ty = ty.as_str();
        let (mut x, mut y) = (jint(c, "x", 0) as i32, jint(c, "y", 0) as i32);
        if let Some(e) = enc {
            x += e.x;
            y += e.y;
        }
        let mut cx = VarCtx { env: self.env, vars: self.vars, enc, processor, cache: HashMap::new() };
        let str_field = |cx: &mut VarCtx<'_>, key: &str| -> String {
            as_string(&lookup(cx, &Value::String(jstr(c, key).unwrap_or("").to_owned())))
        };
        let group = str_field(&mut cx, "group");
        let flag = str_field(&mut cx, "flag");
        let advancement = str_field(&mut cx, "advancement");
        let mut guard_pass = match jstr(c, "guard") {
            None => true,
            Some(g) => as_bool(&lookup(&mut cx, &Value::String(g.to_owned()))),
        };
        // getVisibleStatus: the processor may hide a group
        if let Some(p) = processor {
            if !group.is_empty() && !p.allow_render(&group) {
                guard_pass = false;
            }
        }
        let field = |cx: &mut VarCtx<'_>, key: &str| lookup(cx, c.get(key).unwrap_or(&Value::Null));
        let kind = match ty {
            "patchouli:text" => CompKind::Text {
                text: component_text(&field(&mut cx, "text")),
                color: hex_color(&field(&mut cx, "color")),
                max_width: jint(c, "max_width", i64::from(PAGE_WIDTH)) as i32,
                line_height: jint(c, "line_height", i64::from(TEXT_LINE_HEIGHT)) as i32,
            },
            "patchouli:item" => {
                let items = stacks_of(self.env, &field(&mut cx, "item"));
                CompKind::Item { items, framed: jbool(c, "framed", false), link_recipe: jbool(c, "link_recipe", false) }
            }
            "patchouli:image" => CompKind::Image {
                image: str_field(&mut cx, "image"),
                u: jint(c, "u", 0) as i32,
                v: jint(c, "v", 0) as i32,
                width: jint(c, "width", 0) as i32,
                height: jint(c, "height", 0) as i32,
                tw: jint(c, "texture_width", 256) as i32,
                th: jint(c, "texture_height", 256) as i32,
                scale: jfloat(c, "scale", 1.0) as f32,
            },
            "patchouli:header" => {
                if x == -1 {
                    x = PAGE_WIDTH / 2;
                }
                if y == -1 {
                    y = 0;
                }
                CompKind::Header {
                    text: component_text(&field(&mut cx, "text")),
                    color: hex_color(&field(&mut cx, "color")),
                    centered: jbool(c, "centered", true),
                    scale: jfloat(c, "scale", 1.0) as f32,
                }
            }
            "patchouli:separator" => {
                if x == -1 {
                    x = 0;
                }
                if y == -1 {
                    y = 12;
                }
                CompKind::Separator
            }
            "patchouli:frame" => {
                if x == -1 {
                    x = PAGE_WIDTH / 2 - 53;
                }
                if y == -1 {
                    y = 7;
                }
                CompKind::Frame
            }
            "patchouli:entity" => CompKind::Entity {
                id: as_string(&field(&mut cx, "entity")),
                render_size: jfloat(c, "render_size", 100.0) as f32,
                rotate: jbool(c, "rotate", true),
                default_rotation: jfloat(c, "default_rotation", -45.0) as f32,
            },
            "patchouli:tooltip" => {
                let raw = c.get("tooltip").cloned().unwrap_or(Value::Null);
                let items: Vec<Value> = match raw {
                    Value::Array(a) => a,
                    Value::Null => Vec::new(),
                    other => vec![other],
                };
                CompKind::Tooltip {
                    lines: items.iter().map(|i| component_text(&lookup(&mut cx, i))).collect(),
                    width: jint(c, "width", 0) as i32,
                    height: jint(c, "height", 0) as i32,
                }
            }
            other => CompKind::Unknown(other.to_owned()),
        };
        Some(Comp {
            x,
            y,
            group,
            flag,
            advancement,
            negate_advancement: jbool(c, "negate_advancement", false),
            guard_pass,
            kind,
        })
    }
}

/// The components of a template page. `Err` when the template does not exist or is broken.
pub fn compile_template_page(env: &dyn Env, contents: &Contents, info: &BookInfo, page: &Page) -> Result<Vec<Comp>, String> {
    let comp = Compiler { env, contents, info, vars: &page.json };
    let tpl = comp.template(jstr(&page.json, "type").unwrap_or(&page.kind))?;
    comp.compile(tpl, None)
}

/// The stacks a page explains (`BookPage.build` and `addRelevantStack`).
pub fn relevant_stacks(env: &dyn Env, contents: &Contents, info: &BookInfo, page: &Page) -> Vec<Stack> {
    if let Some(kind) = RecipeKind::of_page(&page.kind) {
        let mut out = Vec::new();
        for key in ["recipe", "recipe2"] {
            if let Some(id) = jstr(&page.json, key) {
                if let Some(r) = load_recipe(env, kind, id) {
                    out.push(r.result);
                }
            }
        }
        return out;
    }
    match page.kind.as_str() {
        "patchouli:spotlight" => {
            if jbool(&page.json, "link_recipe", false) {
                stacks_of(env, page.json.get("item").unwrap_or(&Value::Null))
            } else {
                Vec::new()
            }
        }
        k if k.starts_with("patchouli:") && page.is_builtin() => {
            let _ = k;
            Vec::new()
        }
        _ => match compile_template_page(env, contents, info, page) {
            Ok(comps) => comps
                .into_iter()
                .filter_map(|c| match c.kind {
                    CompKind::Item { items, link_recipe: true, .. } => Some(items),
                    _ => None,
                })
                .flatten()
                .collect(),
            Err(_) => Vec::new(),
        },
    }
}

// ------------------------------------------------------------------------------------------------
// The screens (GuiBookLanding, GuiBookEntryList/Index/History/Category, GuiBookEntry) as a list of elements
// ------------------------------------------------------------------------------------------------

/// The screens of a book (`GuiBook` subclasses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gui {
    Landing,
    Index,
    History,
    Category(String),
    Entry(String),
}
/// A screen and its spread (`GuiBook.spread`: the pair of pages shown).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Where {
    pub gui: Gui,
    pub spread: usize,
}
impl Where {
    pub fn landing() -> Self {
        Self { gui: Gui::Landing, spread: 0 }
    }
    pub fn entry(id: &str, spread: usize) -> Self {
        Self { gui: Gui::Entry(id.to_owned()), spread }
    }
}

/// What a book remembers while it is not closed for good (`BookContents.currentGui` and `guiStack`).
#[derive(Clone, Debug)]
pub struct Nav {
    pub cur: Where,
    pub stack: Vec<Where>,
    /// The text typed into the search bar of a list.
    pub search: String,
    /// The picture shown by each image page, by its place in the entry.
    pub image_index: BTreeMap<usize, usize>,
}
impl Default for Nav {
    fn default() -> Self {
        Self { cur: Where::landing(), stack: Vec::new(), search: String::new(), image_index: BTreeMap::new() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tex {
    /// The book's texture (512 x 256).
    Book,
    /// The crafting texture (128 x 256).
    Crafting,
    /// The page filler (128 x 128).
    Filler,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Icon {
    Stack(Stack),
    /// `ns:textures/....png`, 16 x 16.
    Texture(String),
}
impl Icon {
    /// `BookIcon.from`.
    pub fn from_str(s: &str) -> Icon {
        if s.ends_with(".png") { Icon::Texture(qualify("minecraft", s)) } else { Icon::Stack(parse_stack(s).unwrap_or_default()) }
    }
}

/// What a click does.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// `displayLexiconGui(gui, true)`.
    Open(Where),
    /// `GuiBookEntry.displayOrBookmark`: Shift bookmarks it instead.
    Entry(String),
    Back,
    /// A page arrow (`true` = the left one).
    Page(bool),
    /// A bookmark button: the index in the book's bookmarks, or `None` for the one that adds.
    Bookmark(Option<usize>),
    MarkRead,
    ToggleQuest(String),
    Image { page: usize, left: bool },
    Link(Link),
    /// The list of books: open that book (the game's side does it).
    OpenBook(String),
}

/// Things the game's side has to do after a click.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// `GuiBook.playBookFlipSound`.
    Flip,
    OpenUrl(String),
    Command(String),
    /// The reading state changed: save it.
    Save,
    Close,
}

/// One thing to draw. Coordinates are in GUI pixels from the top left corner of the book.
#[derive(Clone, Debug, PartialEq)]
pub enum Elem {
    /// `fill(x1, y1, x2, y2, argb)`.
    Fill { rect: [i32; 4], argb: u32 },
    /// `fillGradient(x1, y1, x2, y2, top, bottom)`.
    Gradient { rect: [i32; 4], top: u32, bottom: u32 },
    /// `blit(tex, x, y, u, v, w, h, tw, th)`; the source region `sw` x `sh` is stretched over `w` x `h`.
    Blit { tex: Tex, x: i32, y: i32, w: i32, h: i32, u: i32, v: i32, sw: i32, sh: i32, tw: i32, th: i32, alpha: f32 },
    /// A texture of the game by its resource id (`ns:textures/gui/x.png`).
    Image { texture: String, x: i32, y: i32, w: i32, h: i32, u: i32, v: i32, sw: i32, sh: i32, tw: i32, th: i32 },
    /// `drawString(font, text, x, y, color, shadow)`.
    Str { text: String, x: i32, y: i32, color: u32, scale: f32, uniform: bool, shadow: bool },
    Words { text: BookText, uniform: bool },
    /// An item (or the stacks that take turns), 16 x 16 times `scale`; hovering gives its tooltip.
    Item { stacks: Vec<Stack>, x: i32, y: i32, scale: f32 },
    Icon { icon: Icon, x: i32, y: i32, scale: f32, alpha: f32 },
    /// `GuiButtonBook`: the picture is at `u`, `v` of the book texture and `u + w` when hovered.
    Button { rect: [i32; 4], u: i32, v: i32, tip: Vec<String>, cmd: Cmd },
    EntryBtn { x: i32, y: i32, label: String, locked: bool, priority: bool, argb: u32, icon: Icon, state: ReadState, cmd: Cmd },
    CatBtn { x: i32, y: i32, name: String, icon: Icon, locked: bool, state: Option<ReadState>, cmd: Cmd },
    Bookmark { x: i32, y: i32, tip: Vec<String>, icon: Option<Icon>, number: String, kind: i32, cmd: Cmd },
    MarkRead { x: i32, y: i32, tip: String, cmd: Cmd },
    /// A vanilla button (the link and quest pages).
    PlainButton { rect: [i32; 4], text: String, cmd: Cmd },
    /// A place that shows a tooltip when hovered; `rect` is x, y, w, h.
    Tip { rect: [i32; 4], lines: Vec<String> },
    /// An entity at the centre `x`, `y` (`PageEntity.renderEntity`): drawn `render_size / max(1, width, height) * 0.8`
    /// times as big as its model, moved up by `extra_offset`.
    Entity { id: String, x: i32, y: i32, render_size: f32, extra_offset: f32, rotate: bool, default_rotation: f32 },
    /// A multiblock drawn from its blocks: (x, y, z, block id), the size of the box, the centre and the room it has.
    Blocks { blocks: Vec<(i32, i32, i32, String)>, size: [i32; 3], x: i32, y: i32, room: i32 },
}

/// A screen ready to be drawn.
#[derive(Clone, Debug, Default)]
pub struct View {
    pub elems: Vec<Elem>,
    pub max_spreads: usize,
    /// A list with a search bar that takes typed keys.
    pub searchable: bool,
    /// The only entry of a search (Enter opens it).
    pub single_result: Option<String>,
    /// The book this view is for (a book is also given the pages of its screens by id).
    pub title: String,
}

// ------------------------------------------------------------------------------------------------
// Pages of an entry
// ------------------------------------------------------------------------------------------------

/// `BookEntry.getPages`: the pages that are unlocked (all of them when the book has no locking); one empty page if
/// nothing is left.
pub fn visible_pages(env: &dyn Env, book: &BookInfo, entry: &Entry) -> Vec<Page> {
    let mut pages: Vec<Page> = if book.advancements_enabled() {
        entry.pages.iter().filter(|p| p.advancement.as_deref().is_none_or(|a| env.advancement_done(a))).cloned().collect()
    } else {
        entry.pages.clone()
    };
    if pages.is_empty() {
        pages.push(Page {
            index: 0,
            kind: "patchouli:empty".into(),
            json: serde_json::json!({"type": "patchouli:empty"}),
            anchor: None,
            advancement: None,
        });
    }
    pages
}

/// What the screens read: the game, the book and what the player did in it.
pub struct Cx<'a> {
    pub env: &'a dyn Env,
    pub book: &'a Book,
    pub persist: &'a Persist,
}
impl<'a> Cx<'a> {
    pub fn new(env: &'a dyn Env, book: &'a Book, persist: &'a Persist) -> Self {
        Self { env, book, persist }
    }
    pub fn text_cx(&self) -> TextCx<'a> {
        TextCx { env: self.env, book: &self.book.info, contents: &self.book.contents }
    }
    fn info(&self) -> &BookInfo {
        &self.book.info
    }
    fn contents(&self) -> &Contents {
        &self.book.contents
    }
    fn uniform(&self) -> bool {
        !self.book.info.use_blocky_font
    }
    fn tr(&self, key: &str, fallback: &str) -> String {
        self.env.translate(key).unwrap_or_else(|| fallback.to_owned())
    }
    /// `Font.width` in the font of the screen: the plain one. Only the texts that the book styles (its text, the names of the
    /// entry buttons, the subtitle, the search bar) are in the book's own font.
    fn width(&self, s: &str) -> i32 {
        text_width(self.env, s, false, false)
    }
    fn gray(&self, s: String) -> String {
        format!("§7{s}")
    }
    /// `drawCenteredStringNoShadow(font, s, x, y, color)`.
    fn centered(&self, out: &mut Vec<Elem>, s: &str, x: i32, y: i32, color: u32) {
        let w = self.width(s);
        out.push(Elem::Str { text: s.to_owned(), x: x - w / 2, y, color, scale: 1.0, uniform: false, shadow: false });
    }
    fn string(&self, out: &mut Vec<Elem>, s: &str, x: i32, y: i32, color: u32) {
        out.push(Elem::Str { text: s.to_owned(), x, y, color, scale: 1.0, uniform: false, shadow: false });
    }
    /// A text laid out at book coordinates (`x`, `y` is where its first line goes).
    fn text(&self, out: &mut Vec<Elem>, markup: &str, x: i32, y: i32, width: i32, line_height: i32, color: u32) {
        let t = BookText::new(&self.text_cx(), markup, x, y, width, line_height, color);
        out.push(Elem::Words { text: t, uniform: self.uniform() });
    }
    /// A text that is already translated (the landing text, the descriptions of the lists).
    fn text_translated(&self, out: &mut Vec<Elem>, markup: &str, x: i32, y: i32) {
        let t = BookText::from_markup(&self.text_cx(), markup, x, y, PAGE_WIDTH, TEXT_LINE_HEIGHT, self.info().text_color);
        out.push(Elem::Words { text: t, uniform: self.uniform() });
    }
    fn name(&self, s: &str) -> String {
        display_name(self.env, self.info(), s)
    }
}

fn blit(out: &mut Vec<Elem>, tex: Tex, x: i32, y: i32, u: i32, v: i32, w: i32, h: i32) {
    let (tw, th) = match tex {
        Tex::Book => (512, 256),
        Tex::Crafting => (128, 256),
        Tex::Filler => (128, 128),
    };
    out.push(Elem::Blit { tex, x, y, w, h, u, v, sw: w, sh: h, tw, th, alpha: 1.0 });
}

/// `GuiBook.drawSeparator`.
fn separator(out: &mut Vec<Elem>, x: i32, y: i32) {
    let (w, h) = (110, 3);
    out.push(Elem::Blit { tex: Tex::Book, x: x + PAGE_WIDTH / 2 - w / 2, y, w, h, u: 140, v: 180, sw: w, sh: h, tw: 512, th: 256, alpha: 0.8 });
}

/// `GuiBook.drawPageFiller(x, y)`.
fn page_filler(out: &mut Vec<Elem>, x: i32, y: i32) {
    out.push(Elem::Blit {
        tex: Tex::Filler,
        x: x + PAGE_WIDTH / 2 - 64,
        y: y + PAGE_HEIGHT / 2 - 74,
        w: 128,
        h: 128,
        u: 0,
        v: 0,
        sw: 128,
        sh: 128,
        tw: 128,
        th: 128,
        alpha: 1.0,
    });
}

/// `java.awt.Color.darker`.
fn darker(rgb: u32) -> u32 {
    let ch = |s: u32| (((rgb >> s) & 255) as f32 * 0.7) as u32;
    ch(16) << 16 | ch(8) << 8 | ch(0)
}

impl Cx<'_> {
    fn entry_btn(&self, out: &mut Vec<Elem>, x: i32, y: i32, id: &str) {
        let Some(e) = self.contents().entries.get(id) else { return };
        let locked = self.contents().entry_locked(self.env, self.info(), id);
        let (argb, label) = if locked {
            (0x7700_0000 | self.info().text_color, self.tr("patchouli.gui.lexicon.locked", "(Locked)"))
        } else if e.secret {
            (0xAA00_0000 | self.info().text_color, self.name(&e.name))
        } else {
            (0xFF00_0000 | e.entry_color.unwrap_or(self.info().text_color), self.name(&e.name))
        };
        out.push(Elem::EntryBtn {
            x,
            y,
            label,
            locked,
            priority: e.priority && !locked,
            argb,
            icon: Icon::from_str(&e.icon),
            state: self.contents().entry_state(self.env, self.persist, self.info(), id),
            cmd: Cmd::Entry(id.to_owned()),
        });
    }
    fn category_btn(&self, out: &mut Vec<Elem>, x: i32, y: i32, id: &str) {
        let Some(c) = self.contents().categories.get(id) else { return };
        let locked = self.contents().category_locked(self.env, self.info(), id);
        out.push(Elem::CatBtn {
            x,
            y,
            name: self.name(&c.name),
            icon: Icon::from_str(&c.icon),
            locked,
            state: (!locked).then(|| self.contents().category_state(self.env, self.persist, self.info(), id)),
            cmd: Cmd::Open(Where { gui: Gui::Category(id.to_owned()), spread: 0 }),
        });
    }
}

// ------------------------------------------------------------------------------------------------
// Lists
// ------------------------------------------------------------------------------------------------

/// `GuiBookEntryList.getEntryCountStart`.
fn list_start(spread: usize) -> usize {
    if spread == 0 { 0 } else { ENTRIES_IN_FIRST_PAGE + ENTRIES_PER_PAGE * 2 * (spread - 1) }
}

impl Cx<'_> {
    /// The entries a list shows, before the search: `getEntries()` without the hidden ones, sorted.
    fn list_entries(&self, gui: &Gui) -> Vec<String> {
        let c = self.contents();
        let mut v: Vec<String> = match gui {
            Gui::Index => c.entries.keys().cloned().collect(),
            Gui::Category(id) => c.categories.get(id).map(|cat| cat.entries.clone()).unwrap_or_default(),
            Gui::History => self
                .persist
                .get(&self.info().id)
                .map(|d| d.history.iter().filter(|e| c.entries.contains_key(*e) && !c.entry_locked(self.env, self.info(), e)).cloned().collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        v.retain(|e| !c.entry_hidden(self.env, self.info(), e));
        if !matches!(gui, Gui::History) {
            v.sort_by(|a, b| c.compare_entries(self.env, self.persist, self.info(), a, b));
        }
        v
    }
    /// `BookEntry.isFoundByQuery`.
    fn found_by(&self, id: &str, query: &str) -> bool {
        let Some(e) = self.contents().entries.get(id) else { return false };
        if self.name(&e.name).to_lowercase().contains(query) {
            return true;
        }
        e.relevant.iter().any(|s| self.env.item_name(s).to_lowercase().contains(query))
    }
    fn visible_list(&self, gui: &Gui, query: &str) -> Vec<String> {
        let query = query.to_lowercase();
        self.list_entries(gui).into_iter().filter(|e| self.found_by(e, &query)).collect()
    }
    fn list_max_spreads(&self, visible: usize) -> usize {
        let mut max = 1;
        if visible > ENTRIES_IN_FIRST_PAGE {
            max += (visible - ENTRIES_IN_FIRST_PAGE).div_ceil(ENTRIES_PER_PAGE * 2);
        }
        max
    }
    /// `GuiBookCategory.addSubcategoryButtons`' categories.
    fn subcategories(&self, parent: &str) -> Vec<String> {
        let c = self.contents();
        let mut v: Vec<String> = c
            .categories
            .values()
            .filter(|cat| cat.parent.as_deref() == Some(parent) && !c.category_hidden(self.env, self.info(), &cat.id))
            .map(|cat| cat.id.clone())
            .collect();
        v.sort_by(|a, b| c.compare_categories(self.env, self.info(), a, b));
        v
    }
}

// ------------------------------------------------------------------------------------------------
// The view
// ------------------------------------------------------------------------------------------------

/// Brings a navigation back into shape after a change: a search that leaves fewer entries pulls the spread back.
pub fn normalize(env: &dyn Env, book: &Book, persist: &Persist, nav: &mut Nav) {
    let cx = Cx::new(env, book, persist);
    match &nav.cur.gui {
        Gui::Index | Gui::History | Gui::Category(_) => {
            let n = cx.visible_list(&nav.cur.gui, &nav.search).len();
            while list_start(nav.cur.spread) > n && nav.cur.spread > 0 {
                nav.cur.spread -= 1;
            }
        }
        Gui::Entry(id) => {
            if let Some(e) = book.contents.entries.get(id) {
                let max = visible_pages(env, &book.info, e).len().div_ceil(2);
                nav.cur.spread = nav.cur.spread.min(max.saturating_sub(1));
            }
        }
        Gui::Landing => nav.cur.spread = 0,
    }
}

/// The elements of the current screen of `nav`.
pub fn build_view(env: &dyn Env, book: &Book, persist: &Persist, nav: &Nav) -> View {
    let cx = Cx::new(env, book, persist);
    let mut out: Vec<Elem> = Vec::new();
    // GuiBook.drawBackgroundElements
    out.push(Elem::Blit {
        tex: Tex::Book,
        x: 0,
        y: 0,
        w: FULL_WIDTH,
        h: FULL_HEIGHT,
        u: 0,
        v: 0,
        sw: FULL_WIDTH,
        sh: FULL_HEIGHT,
        tw: 512,
        th: 256,
        alpha: 1.0,
    });
    let mut view = View { title: book.info.title(env), ..View::default() };
    match &nav.cur.gui {
        Gui::Landing => landing(&cx, nav, &mut out, &mut view),
        Gui::Index | Gui::History | Gui::Category(_) => entry_list(&cx, nav, &mut out, &mut view),
        Gui::Entry(id) => entry_screen(&cx, nav, id, &mut out, &mut view),
    }
    common_buttons(&cx, nav, &mut out, &view);
    view.elems = out;
    view
}

/// The buttons every screen has: back, the arrows, the bookmarks and "mark as read".
fn common_buttons(cx: &Cx<'_>, nav: &Nav, out: &mut Vec<Elem>, view: &View) {
    let tr = |k: &str, f: &str| cx.tr(k, f);
    if !nav.stack.is_empty() {
        out.push(Elem::Button {
            rect: [FULL_WIDTH / 2 - 9, FULL_HEIGHT - 5, 18, 9],
            u: 308,
            v: 0,
            tip: vec![tr("patchouli.gui.lexicon.button.back", "Back"), cx.gray(tr("patchouli.gui.lexicon.button.back.info", "Shift-Click to return to Home"))],
            cmd: Cmd::Back,
        });
    }
    for left in [true, false] {
        let visible = if left { nav.cur.spread > 0 } else { nav.cur.spread + 1 < view.max_spreads };
        if visible {
            out.push(Elem::Button {
                rect: [if left { -4 } else { FULL_WIDTH - 14 }, FULL_HEIGHT - 6, 18, 10],
                u: 272,
                v: if left { 10 } else { 0 },
                tip: vec![if left { tr("patchouli.gui.lexicon.button.prev_page", "Previous") } else { tr("patchouli.gui.lexicon.button.next_page", "Next") }],
                cmd: Cmd::Page(left),
            });
        }
    }
    // Bookmarks (addBookmarkButtons)
    let marks: Vec<(String, usize)> = cx.persist.get(&cx.info().id).map(|d| d.bookmarks.clone()).unwrap_or_default();
    let mut y = 0;
    for (i, (entry, spread)) in marks.iter().enumerate() {
        let known = cx.contents().entries.get(entry);
        let tip = match known {
            Some(e) => vec![cx.name(&e.name), cx.gray(tr("patchouli.gui.lexicon.remove_bookmark", "(Shift-Click to Remove)"))],
            None => vec![tr("patchouli.gui.lexicon.add_bookmark", "Add Bookmark")],
        };
        out.push(Elem::Bookmark {
            x: FULL_WIDTH,
            y: TOP_PADDING + y,
            tip,
            icon: known.map(|e| Icon::from_str(&e.icon)),
            number: (spread + 1).to_string(),
            kind: 160,
            cmd: Cmd::Bookmark(Some(i)),
        });
        y += 12;
    }
    if let Gui::Entry(id) = &nav.cur.gui {
        let already = marks.iter().any(|(e, s)| e == id && *s == nav.cur.spread);
        if !already && marks.len() <= MAX_BOOKMARKS {
            out.push(Elem::Bookmark {
                x: FULL_WIDTH,
                y: TOP_PADDING + y,
                tip: vec![tr("patchouli.gui.lexicon.add_bookmark", "Add Bookmark")],
                icon: None,
                number: String::new(),
                kind: 170,
                cmd: Cmd::Bookmark(None),
            });
        }
    }
    // shouldAddMarkReadButton
    let on_list = matches!(nav.cur.gui, Gui::Landing | Gui::History | Gui::Category(_));
    if on_list {
        let any_unread = cx.contents().entries.keys().any(|e| {
            !cx.contents().entry_locked(cx.env, cx.info(), e)
                && cx.contents().entry_state(cx.env, cx.persist, cx.info(), e) == ReadState::Unread
        });
        if any_unread {
            let main = nav.stack.is_empty();
            let tip = if main {
                tr("patchouli.gui.lexicon.button.mark_all_read", "Mark all as read")
            } else {
                tr("patchouli.gui.lexicon.button.mark_category_read", "Mark this category as read")
            };
            out.push(Elem::MarkRead { x: FULL_WIDTH, y: TOP_PADDING + PAGE_HEIGHT - 10, tip, cmd: Cmd::MarkRead });
        }
    }
}

/// `GuiBook.drawProgressBar`.
fn progress_bar(cx: &Cx<'_>, out: &mut Vec<Elem>, filter: impl Fn(&Entry) -> bool) {
    let info = cx.info();
    if !info.show_progress || !info.advancements_enabled() {
        return;
    }
    let (bar_left, bar_top, bar_width, bar_height) = (19, FULL_HEIGHT - 36, PAGE_WIDTH - 10, 12);
    let c = cx.contents();
    let (mut total, mut unlocked, mut secret_unlocked) = (0usize, 0usize, 0usize);
    for e in c.entries.values() {
        if !filter(e) {
            continue;
        }
        if e.secret {
            if !c.entry_locked(cx.env, info, &e.id) {
                secret_unlocked += 1;
            }
        } else {
            if c.categories.get(&e.category).is_some_and(|cat| cat.secret) && !c.category_locked(cx.env, info, &e.category) {
                continue;
            }
            total += 1;
            if !c.entry_locked(cx.env, info, &e.id) {
                unlocked += 1;
            }
        }
    }
    let fract = unlocked as f32 / (total.max(1)) as f32;
    let progress_width = ((bar_width - 1) as f32 * fract) as i32;
    out.push(Elem::Fill { rect: [bar_left, bar_top, bar_left + bar_width, bar_top + bar_height], argb: 0xFF00_0000 | info.header_color });
    let g = |color: u32| (0xFF00_0000 | color, 0xFF00_0000 | darker(color));
    let (bt, bb) = g(info.progress_bar_background);
    out.push(Elem::Gradient { rect: [bar_left + 1, bar_top + 1, bar_left + bar_width - 1, bar_top + bar_height - 1], top: bt, bottom: bb });
    let (pt, pb) = g(info.progress_bar_color);
    out.push(Elem::Gradient { rect: [bar_left + 1, bar_top + 1, bar_left + progress_width, bar_top + bar_height - 1], top: pt, bottom: pb });
    cx.string(out, &cx.tr("patchouli.gui.lexicon.progress_meter", "Unlock Progress:"), bar_left, bar_top - 9, info.header_color);
    let mut tip = vec![cx.tr("patchouli.gui.lexicon.progress_tooltip", "%d/%d Unlocked").replacen("%d", &unlocked.to_string(), 1).replacen("%d", &total.to_string(), 1)];
    if secret_unlocked > 0 {
        tip.push(if secret_unlocked == 1 {
            cx.gray(cx.tr("patchouli.gui.lexicon.progress_tooltip.secret1", "Plus 1 Secret"))
        } else {
            cx.gray(cx.tr("patchouli.gui.lexicon.progress_tooltip.secret", "Plus %d Secrets").replace("%d", &secret_unlocked.to_string()))
        });
    }
    if unlocked != total {
        tip.push(cx.gray(cx.tr("patchouli.gui.lexicon.progress_tooltip.info", "Complete Advancements to unlock more!")));
    }
    out.push(Elem::Tip { rect: [bar_left, bar_top, bar_width, bar_height], lines: tip });
}

fn landing(cx: &Cx<'_>, nav: &Nav, out: &mut Vec<Elem>, view: &mut View) {
    let info = cx.info();
    let c = cx.contents();
    let landing_text = cx.env.translate(&info.landing_text).unwrap_or_else(|| info.landing_text.clone());
    cx.text_translated(out, &landing_text, LEFT_PAGE_X, TOP_PADDING + 25);
    let disable_bar = !info.show_progress || !info.advancements_enabled();
    // The row of small buttons under the text.
    let (bx, by, dist) = (if disable_bar { 25 } else { 20 }, FULL_HEIGHT - if disable_bar { 25 } else { 62 }, 15);
    out.push(Elem::Button {
        rect: [bx, by, 11, 11],
        u: 330,
        v: 31,
        tip: vec![cx.tr("patchouli.gui.lexicon.button.history", "History")],
        cmd: Cmd::Open(Where { gui: Gui::History, spread: 0 }),
    });
    let _ = dist;
    let mut loaded = 0usize;
    if c.pamphlet.is_none() {
        let cats = c.root_categories(cx.env, info);
        for (i, id) in cats.iter().enumerate() {
            cx.category_btn(out, RIGHT_PAGE_X + 10 + (i as i32 % 4) * 24, TOP_PADDING + 25 + (i as i32 / 4) * 24, id);
        }
        let i = cats.len() as i32;
        // The entry index, with the icon of the book.
        out.push(Elem::CatBtn {
            x: RIGHT_PAGE_X + 10 + (i % 4) * 24,
            y: TOP_PADDING + 25 + (i / 4) * 24,
            name: cx.tr("patchouli.gui.lexicon.index", "Entry Index"),
            icon: book_icon(cx),
            locked: false,
            state: None,
            cmd: Cmd::Open(Where { gui: Gui::Index, spread: 0 }),
        });
        loaded = cats.len() + 1;
        view.max_spreads = 0;
    } else {
        let mut entries: Vec<String> = c.entries.keys().cloned().collect();
        entries.retain(|e| !c.entry_hidden(cx.env, info, e));
        entries.sort_by(|a, b| c.compare_entries(cx.env, cx.persist, info, a, b));
        if c.errors.is_empty() {
            for (i, id) in entries.iter().take(ENTRIES_IN_FIRST_PAGE).enumerate() {
                cx.entry_btn(out, RIGHT_PAGE_X, TOP_PADDING + i as i32 * 11, id);
            }
        }
        view.max_spreads = 1;
    }
    // drawForegroundElements
    let top_sep = TOP_PADDING + 12;
    let bottom_sep = top_sep + 25 + 24 * ((loaded as i32 - 1).max(0) / 4 + 1);
    // drawHeader
    blit(out, Tex::Book, -8, 12, 0, 180, 140, 31);
    let name = book_title(cx);
    cx.string(out, &name, 13, 16, info.nameplate_color);
    out.push(Elem::Str { text: info.subtitle_text(cx.env), x: 24, y: 24, color: info.nameplate_color, scale: 1.0, uniform: cx.uniform(), shadow: false });
    if c.pamphlet.is_none() {
        cx.centered(out, &cx.tr("patchouli.gui.lexicon.categories", "Categories"), RIGHT_PAGE_X + PAGE_WIDTH / 2, TOP_PADDING, info.header_color);
        separator(out, RIGHT_PAGE_X, top_sep);
        if loaded <= 16 {
            separator(out, RIGHT_PAGE_X, bottom_sep);
        }
    }
    if !c.errors.is_empty() {
        let x = RIGHT_PAGE_X + PAGE_WIDTH / 2;
        let y = bottom_sep + 12;
        cx.centered(out, &cx.tr("patchouli.gui.lexicon.loading_error", "Loading error!"), x, y, 0xFF0000);
        cx.centered(out, &cx.tr("patchouli.gui.lexicon.loading_error_hover", "(Hover for info)"), x, y + 10, 0x777777);
        let mut lines: Vec<String> = c.errors.iter().take(8).cloned().collect();
        if c.errors.len() > 8 {
            lines.push(format!("... {} more", c.errors.len() - 8));
        }
        out.push(Elem::Tip { rect: [x - PAGE_WIDTH / 2, y - 4, PAGE_WIDTH, 20], lines });
    }
    progress_bar(cx, out, |_| true);
    let _ = nav;
}

/// The icon of the book itself: its custom item, or the guide book.
fn book_icon(cx: &Cx<'_>) -> Icon {
    let info = cx.info();
    if !info.index_icon.is_empty() {
        return Icon::from_str(&info.index_icon);
    }
    match info.custom_item() {
        Some(s) => Icon::Stack(s),
        None => Icon::Stack(Stack::new("patchouli:guide_book", 1)),
    }
}
fn book_title(cx: &Cx<'_>) -> String {
    let info = cx.info();
    match info.custom_item() {
        Some(s) if cx.env.item_exists(&s.id) => cx.env.item_name(&s),
        _ => info.title(cx.env),
    }
}

fn entry_list(cx: &Cx<'_>, nav: &Nav, out: &mut Vec<Elem>, view: &mut View) {
    let info = cx.info();
    let c = cx.contents();
    let gui = &nav.cur.gui;
    let spread = nav.cur.spread;
    let category = if let Gui::Category(id) = gui { c.categories.get(id) } else { None };
    let own_entries_empty = category.is_some_and(|cat| cat.entries.is_empty());
    let subs: Vec<String> = if let Gui::Category(id) = gui { cx.subcategories(id) } else { Vec::new() };
    let visible = cx.visible_list(gui, &nav.search);
    view.max_spreads = cx.list_max_spreads(visible.len());
    view.searchable = !(category.is_some() && own_entries_empty);
    if visible.len() == 1 {
        view.single_result = Some(visible[0].clone());
    }
    let title = match gui {
        Gui::Index => cx.tr("patchouli.gui.lexicon.index", "Entry Index"),
        Gui::History => cx.tr("patchouli.gui.lexicon.history", "Reading History"),
        Gui::Category(_) => category.map(|cat| cx.name(&cat.name)).unwrap_or_default(),
        _ => String::new(),
    };
    let chapters_title = if category.is_some() && own_entries_empty && !subs.is_empty() {
        cx.tr("patchouli.gui.lexicon.categories", "Categories")
    } else {
        cx.tr("patchouli.gui.lexicon.chapters", "Chapters")
    };
    // buttons
    if spread == 0 {
        for (i, id) in visible.iter().take(ENTRIES_IN_FIRST_PAGE).enumerate() {
            cx.entry_btn(out, RIGHT_PAGE_X, TOP_PADDING + 20 + i as i32 * 11, id);
        }
        // addSubcategoryButtons
        let right_free = own_entries_empty;
        let (base_x, base_y) = if right_free {
            (RIGHT_PAGE_X + 10, TOP_PADDING + 25)
        } else {
            (LEFT_PAGE_X + 10, TOP_PADDING + PAGE_HEIGHT - (subs.len() as i32 / 4) * 20 - if info.advancements_enabled() { 64 } else { 38 })
        };
        for (i, id) in subs.iter().enumerate() {
            let x = base_x + (i as i32 % 4) * 24;
            let y = base_y + (i as i32 / 4) * if right_free { 24 } else { 20 };
            cx.category_btn(out, x, y, id);
        }
    } else {
        let start = list_start(spread);
        for (i, id) in visible.iter().skip(start).take(ENTRIES_PER_PAGE).enumerate() {
            cx.entry_btn(out, LEFT_PAGE_X, TOP_PADDING + i as i32 * 11, id);
        }
        for (i, id) in visible.iter().skip(start + ENTRIES_PER_PAGE).take(ENTRIES_PER_PAGE).enumerate() {
            cx.entry_btn(out, RIGHT_PAGE_X, TOP_PADDING + i as i32 * 11, id);
        }
    }
    let shown_buttons = if spread == 0 {
        visible.len().min(ENTRIES_IN_FIRST_PAGE) + subs.len()
    } else {
        visible.len().saturating_sub(list_start(spread)).min(ENTRIES_PER_PAGE * 2)
    };
    // drawForegroundElements
    if spread == 0 {
        cx.centered(out, &title, LEFT_PAGE_X + PAGE_WIDTH / 2, TOP_PADDING, info.header_color);
        cx.centered(out, &chapters_title, RIGHT_PAGE_X + PAGE_WIDTH / 2, TOP_PADDING, info.header_color);
        separator(out, LEFT_PAGE_X, TOP_PADDING + 12);
        separator(out, RIGHT_PAGE_X, TOP_PADDING + 12);
        let desc = match gui {
            Gui::Index => cx.tr("patchouli.gui.lexicon.index.info", ""),
            Gui::History => cx.tr("patchouli.gui.lexicon.history.info", ""),
            Gui::Category(_) => category.map(|cat| cat.description.clone()).unwrap_or_default(),
            _ => String::new(),
        };
        cx.text(out, &desc, LEFT_PAGE_X, TOP_PADDING + 22, PAGE_WIDTH, TEXT_LINE_HEIGHT, info.text_color);
        if !matches!(gui, Gui::History) {
            let cat_id = if let Gui::Category(id) = gui { Some(id.clone()) } else { None };
            progress_bar(cx, out, move |e| cat_id.as_ref().is_none_or(|id| &e.category == id));
        }
    } else if spread % 2 == 1 && spread == view.max_spreads - 1 && shown_buttons <= ENTRIES_PER_PAGE {
        page_filler(out, RIGHT_PAGE_X, TOP_PADDING);
    }
    if category.is_some() && own_entries_empty && !subs.is_empty() && subs.len() <= 16 {
        let bottom = TOP_PADDING + 37 + 24 * ((subs.len() as i32 - 1) / 4 + 1);
        separator(out, RIGHT_PAGE_X, bottom);
    }
    if !nav.search.is_empty() && view.searchable {
        // The search bar: its texture, and what was typed.
        let (sx, sy) = (160, 170);
        blit(out, Tex::Book, sx - 8, sy, 140, 183, 99, 14);
        out.push(Elem::Str { text: nav.search.clone(), x: sx + 7, y: sy + 1, color: info.text_color, scale: 1.0, uniform: cx.uniform(), shadow: false });
    }
    if visible.is_empty() {
        if !nav.search.is_empty() {
            let x = RIGHT_PAGE_X + PAGE_WIDTH / 2;
            cx.centered(out, &cx.tr("patchouli.gui.lexicon.no_results", "No Results"), x, 80, 0x333333);
            let sad = cx.tr("patchouli.gui.lexicon.sad", ":(");
            let w = cx.width(&sad);
            // drawn at twice the size (the pose is scaled about the corner of the screen): centred on the right page
            out.push(Elem::Str { text: sad, x: RIGHT_PAGE_X + PAGE_WIDTH / 2 - w, y: 94, color: 0x999999, scale: 2.0, uniform: false, shadow: false });
        } else {
            let msg = if !subs.is_empty() && category.is_some() { String::new() } else { cx.tr("patchouli.gui.lexicon.no_entries", "No Entries") };
            if !msg.is_empty() {
                cx.centered(out, &msg, RIGHT_PAGE_X + PAGE_WIDTH / 2, 80, 0x333333);
            }
        }
    }
}

fn entry_screen(cx: &Cx<'_>, nav: &Nav, id: &str, out: &mut Vec<Elem>, view: &mut View) {
    let Some(entry) = cx.contents().entries.get(id) else { return };
    let pages = visible_pages(cx.env, cx.info(), entry);
    view.max_spreads = pages.len().div_ceil(2);
    let spread = nav.cur.spread;
    let left = pages.get(spread * 2);
    let right = pages.get(spread * 2 + 1);
    if let Some(p) = left {
        page_elems(cx, nav, entry, p, LEFT_PAGE_X, TOP_PADDING, out);
    }
    if let Some(p) = right {
        page_elems(cx, nav, entry, p, RIGHT_PAGE_X, TOP_PADDING, out);
    } else {
        page_filler(out, RIGHT_PAGE_X, TOP_PADDING);
    }
}

// ------------------------------------------------------------------------------------------------
// Pages
// ------------------------------------------------------------------------------------------------

/// One page at (`left`, `top`) of the book.
struct PageOut<'a> {
    cx: &'a Cx<'a>,
    left: i32,
    top: i32,
    out: &'a mut Vec<Elem>,
}
impl PageOut<'_> {
    fn blit(&mut self, tex: Tex, x: i32, y: i32, u: i32, v: i32, w: i32, h: i32) {
        blit(self.out, tex, x + self.left, y + self.top, u, v, w, h);
    }
    fn blit_alpha(&mut self, tex: Tex, x: i32, y: i32, u: i32, v: i32, w: i32, h: i32, alpha: f32) {
        let (tw, th) = match tex {
            Tex::Book => (512, 256),
            Tex::Crafting => (128, 256),
            Tex::Filler => (128, 128),
        };
        self.out.push(Elem::Blit { tex, x: x + self.left, y: y + self.top, w, h, u, v, sw: w, sh: h, tw, th, alpha });
    }
    fn centered(&mut self, s: &str, x: i32, y: i32) {
        let color = self.cx.info().header_color;
        self.cx.centered(self.out, s, x + self.left, y + self.top, color);
    }
    fn centered_color(&mut self, s: &str, x: i32, y: i32, color: u32) {
        self.cx.centered(self.out, s, x + self.left, y + self.top, color);
    }
    fn separator(&mut self, x: i32, y: i32) {
        separator(self.out, x + self.left, y + self.top);
    }
    fn text(&mut self, markup: &str, y: i32) {
        self.text_at(markup, 0, y, PAGE_WIDTH, TEXT_LINE_HEIGHT, self.cx.info().text_color);
    }
    fn text_at(&mut self, markup: &str, x: i32, y: i32, width: i32, line_height: i32, color: u32) {
        self.cx.text(self.out, markup, x + self.left, y + self.top, width, line_height, color);
    }
    fn item(&mut self, stacks: Vec<Stack>, x: i32, y: i32) {
        if !stacks.is_empty() {
            self.out.push(Elem::Item { stacks, x: x + self.left, y: y + self.top, scale: 1.0 });
        }
    }
    fn i18n(&self, s: &str) -> String {
        self.cx.text_cx().i18n(s)
    }
    fn tip(&mut self, x: i32, y: i32, w: i32, h: i32, lines: Vec<String>) {
        self.out.push(Elem::Tip { rect: [x + self.left, y + self.top, w, h], lines });
    }
    fn button(&mut self, x: i32, y: i32, u: i32, v: i32, w: i32, h: i32, tip: Vec<String>, cmd: Cmd) {
        self.out.push(Elem::Button { rect: [x + self.left, y + self.top, w, h], u, v, tip, cmd });
    }
    fn plain_button(&mut self, x: i32, y: i32, w: i32, h: i32, text: String, cmd: Cmd) {
        self.out.push(Elem::PlainButton { rect: [x + self.left, y + self.top, w, h], text, cmd });
    }
}

fn page_text_of(json: &Value) -> String {
    match json.get("text") {
        Some(v) => component_text(v),
        None => String::new(),
    }
}

fn page_elems(cx: &Cx<'_>, nav: &Nav, entry: &Entry, page: &Page, left: i32, top: i32, out: &mut Vec<Elem>) {
    let mut p = PageOut { cx, left, top, out };
    let j = &page.json;
    let info = cx.info();
    let page_num = page.index;
    match page.kind.as_str() {
        "patchouli:text" | "patchouli:link" => {
            let title = jstr(j, "title").unwrap_or("");
            let text_height = if page_num == 0 {
                22
            } else if !title.is_empty() {
                12
            } else {
                -4
            };
            p.text(&page_text_of(j), text_height);
            if page_num == 0 {
                let smol = if cx.env.advanced_tooltips() { entry.id.clone() } else { String::new() };
                let mut rendered_smol = false;
                if !smol.is_empty() {
                    let w = cx.width(&smol);
                    // 0.5 scale, centred on x = PAGE_WIDTH / 2
                    p.out.push(Elem::Str {
                        text: smol,
                        x: left + PAGE_WIDTH / 2 - w / 4,
                        y: top + 6,
                        color: info.header_color,
                        scale: 0.5,
                        uniform: false,
                        shadow: false,
                    });
                    rendered_smol = true;
                }
                let name = cx.name(&entry.name);
                p.centered(&name, PAGE_WIDTH / 2, if rendered_smol { -3 } else { 0 });
                p.separator(0, 12);
            } else if !title.is_empty() {
                let t = p.i18n(title);
                p.centered(&t, PAGE_WIDTH / 2, 0);
            }
            if page.kind == "patchouli:link" {
                let text = component_text(j.get("link_text").unwrap_or(&Value::Null));
                let label = p.i18n(&text);
                let url = jstr(j, "url").unwrap_or("").to_owned();
                p.plain_button(PAGE_WIDTH / 2 - 50, PAGE_HEIGHT - 35, 100, 20, label, Cmd::Link(Link::Url(url)));
            }
        }
        "patchouli:spotlight" => {
            let items = stacks_of(cx.env, j.get("item").unwrap_or(&Value::Null));
            let (w, h) = (66, 26);
            p.blit_alpha(Tex::Crafting, PAGE_WIDTH / 2 - w / 2, 10, 0, 128 - h, w, h, 1.0);
            let title = jstr(j, "title").unwrap_or("");
            let shown = if !title.is_empty() {
                p.i18n(title)
            } else {
                items.first().map(|s| cx.env.item_name(s)).unwrap_or_default()
            };
            p.centered(&shown, PAGE_WIDTH / 2, 0);
            p.item(items, PAGE_WIDTH / 2 - 8, 15);
            p.text(&page_text_of(j), 40);
        }
        "patchouli:empty" => {
            if jbool(j, "draw_filler", true) {
                page_filler(p.out, left, top);
            }
        }
        "patchouli:image" => {
            let images: Vec<String> = match j.get("images") {
                Some(Value::Array(a)) => a.iter().map(as_string).collect(),
                Some(Value::String(s)) => vec![s.clone()],
                _ => Vec::new(),
            };
            let index = nav.image_index.get(&page_num).copied().unwrap_or(0).min(images.len().saturating_sub(1));
            let (x, y) = (PAGE_WIDTH / 2 - 53, 7);
            if let Some(img) = images.get(index) {
                // drawn at half size: a 200 x 200 piece of a 256 x 256 texture
                p.out.push(Elem::Image {
                    texture: qualify("minecraft", img),
                    x: left + x + 3,
                    y: top + y + 3,
                    w: 100,
                    h: 100,
                    u: 0,
                    v: 0,
                    sw: 200,
                    sh: 200,
                    tw: 256,
                    th: 256,
                });
            }
            let border = jbool(j, "border", false);
            if border {
                p.blit(Tex::Book, x, y, 405, 149, 106, 106);
            }
            let title = jstr(j, "title").unwrap_or("");
            if !title.is_empty() {
                let t = p.i18n(title);
                p.centered(&t, PAGE_WIDTH / 2, -3);
            }
            if images.len() > 1 && border {
                let (xs, ys) = (x + 83, y + 92);
                p.out.push(Elem::Fill { rect: [left + xs, top + ys, left + xs + 20, top + ys + 11], argb: 0x4400_0000 });
                p.out.push(Elem::Fill { rect: [left + xs - 1, top + ys - 1, left + xs + 20, top + ys + 11], argb: 0x4400_0000 });
            }
            p.text(&page_text_of(j), 120);
            if images.len() > 1 {
                let tip = vec![cx.tr("patchouli.gui.lexicon.button.prev_page", "Previous")];
                if index > 0 {
                    p.button(90, 100, 272, 27, 5, 7, tip, Cmd::Image { page: page_num, left: true });
                }
                if index + 1 < images.len() {
                    p.button(100, 100, 272, 20, 5, 7, vec![cx.tr("patchouli.gui.lexicon.button.next_page", "Next")], Cmd::Image { page: page_num, left: false });
                }
            }
        }
        "patchouli:entity" => {
            let (x, y) = (PAGE_WIDTH / 2 - 53, 7);
            p.blit(Tex::Book, x, y, 405, 149, 106, 106);
            let id = jstr(j, "entity").unwrap_or("").to_owned();
            let name = match jstr(j, "name") {
                Some(n) if !n.is_empty() => n.to_owned(),
                _ => cx.env.entity_name(&qualify("minecraft", id.split('{').next().unwrap_or(""))).unwrap_or_default(),
            };
            p.centered(&name, PAGE_WIDTH / 2, 0);
            let scale = jfloat(j, "scale", 1.0) as f32;
            p.out.push(Elem::Entity {
                id,
                x: left + 58,
                y: top + 60,
                render_size: 100.0 * scale,
                extra_offset: jfloat(j, "offset", 0.0) as f32,
                rotate: jbool(j, "rotate", true),
                default_rotation: jfloat(j, "default_rotation", -45.0) as f32,
            });
            p.text(&page_text_of(j), 115);
        }
        "patchouli:multiblock" => {
            let (x, y) = (PAGE_WIDTH / 2 - 53, 7);
            p.blit(Tex::Book, x, y, 405, 149, 106, 106);
            let name = jstr(j, "name").unwrap_or("");
            let name = p.i18n(name);
            p.centered(&name, PAGE_WIDTH / 2, 0);
            if let Some(mb) = j.get("multiblock").and_then(multiblock_blocks) {
                p.out.push(Elem::Blocks { blocks: mb.0, size: mb.1, x: left + PAGE_WIDTH / 2, y: top + 60, room: 90 });
            }
            p.text(&page_text_of(j), 115);
        }
        "patchouli:relations" => {
            let title = jstr(j, "title").unwrap_or("");
            let t = if title.is_empty() { cx.tr("patchouli.gui.lexicon.relations", "Related Chapters") } else { p.i18n(title) };
            let mut ids: Vec<String> = j
                .get("entries")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|e| e.as_str().map(|s| qualify(&info.ns, s))).collect())
                .unwrap_or_default();
            let n = ids.len();
            ids.retain(|e| cx.contents().entries.contains_key(e) && !cx.contents().entry_hidden(cx.env, info, e));
            ids.sort_by(|a, b| cx.contents().compare_entries(cx.env, cx.persist, info, a, b));
            for (i, id) in ids.iter().enumerate() {
                cx.entry_btn(p.out, left, top + 20 + i as i32 * 11, id);
            }
            p.centered(&t, PAGE_WIDTH / 2, 0);
            p.separator(0, 12);
            p.text(&page_text_of(j), 22 + n as i32 * 11);
        }
        "patchouli:quest" => {
            let manual = jstr(j, "trigger").is_none();
            p.text(&page_text_of(j), 22);
            let title = jstr(j, "title").unwrap_or("");
            let t = if title.is_empty() { cx.tr("patchouli.gui.lexicon.objective", "Objective") } else { p.i18n(title) };
            p.centered(&t, PAGE_WIDTH / 2, 0);
            p.separator(0, 12);
            let completed = cx.contents().quest_completed(cx.env, cx.persist, info, &entry.id, page);
            if manual {
                let label = if completed {
                    cx.tr("patchouli.gui.lexicon.mark_incomplete", "Mark Incomplete")
                } else {
                    cx.tr("patchouli.gui.lexicon.mark_complete", "Mark Complete")
                };
                p.plain_button(PAGE_WIDTH / 2 - 50, PAGE_HEIGHT - 35, 100, 20, label, Cmd::ToggleQuest(entry.id.clone()));
            } else {
                p.separator(0, PAGE_HEIGHT - 25);
                let s = if completed { cx.tr("patchouli.gui.lexicon.complete", "Complete!") } else { cx.tr("patchouli.gui.lexicon.incomplete", "Incomplete") };
                p.centered_color(&s, PAGE_WIDTH / 2, PAGE_HEIGHT - 17, if completed { 0x008b1a } else { info.header_color });
            }
        }
        k => {
            if let Some(kind) = RecipeKind::of_page(k) {
                recipe_page(&mut p, kind, page);
            } else {
                template_page(&mut p, entry, page);
            }
        }
    }
}

/// The blocks of a multiblock JSON (`SerializedMultiblock`): positions (x, y, z with y up) and block ids, and the size.
pub fn multiblock_blocks(v: &Value) -> Option<(Vec<(i32, i32, i32, String)>, [i32; 3])> {
    let mapping: BTreeMap<char, String> = v
        .get("mapping")
        .and_then(Value::as_object)?
        .iter()
        .filter_map(|(k, val)| Some((k.chars().next()?, val.as_str()?.to_owned())))
        .collect();
    let block_of = |c: char| -> Option<String> {
        let m = mapping.get(&c).cloned().or_else(|| if c == ' ' || c == '0' || c == '_' { None } else { None })?;
        let m = m.trim();
        if m == "AIR" || m == "ANY" || m.starts_with('#') {
            return None;
        }
        let id = m.split(['[', '{']).next().unwrap_or("");
        Some(qualify("minecraft", id))
    };
    let mut blocks = Vec::new();
    let mut size = [0i32; 3];
    if let Some(Value::Array(layers)) = v.get("pattern") {
        let ly = layers.len() as i32;
        size[1] = ly;
        for (i, layer) in layers.iter().enumerate() {
            let rows = layer.as_array()?;
            size[0] = size[0].max(rows.len() as i32);
            for (x, row) in rows.iter().enumerate() {
                let row: Vec<char> = row.as_str()?.chars().collect();
                size[2] = size[2].max(row.len() as i32);
                for (z, c) in row.iter().enumerate() {
                    if let Some(b) = block_of(*c) {
                        blocks.push((x as i32, ly - 1 - i as i32, z as i32, b));
                    }
                }
            }
        }
    } else if let Some(Value::Object(sparse)) = v.get("sparse_pattern") {
        for (k, positions) in sparse {
            let Some(c) = k.chars().next() else { continue };
            let Some(b) = block_of(c) else { continue };
            for p in positions.as_array().into_iter().flatten() {
                let a: Vec<i32> = p.as_array().into_iter().flatten().filter_map(|n| n.as_i64().map(|n| n as i32)).collect();
                if let [x, y, z] = a[..] {
                    blocks.push((x, y, z, b.clone()));
                    size = [size[0].max(x + 1), size[1].max(y + 1), size[2].max(z + 1)];
                }
            }
        }
    } else {
        return None;
    }
    Some((blocks, size))
}

fn recipe_page(p: &mut PageOut<'_>, kind: RecipeKind, page: &Page) {
    let cx = p.cx;
    let j = &page.json;
    let r1 = jstr(j, "recipe").and_then(|id| load_recipe(cx.env, kind, id));
    let r2 = jstr(j, "recipe2").and_then(|id| load_recipe(cx.env, kind, id));
    let (r1, r2) = if r1.is_none() && r2.is_some() { (r2, None) } else { (r1, r2) };
    let custom = jstr(j, "title").filter(|t| !t.is_empty());
    let title1 = match (custom, &r1) {
        (Some(t), _) => p.i18n(t),
        (None, Some(r)) => cx.env.item_name(&r.result),
        (None, None) => String::new(),
    };
    let mut title2 = "-".to_owned();
    if let Some(r) = &r2 {
        title2 = if custom.is_none() { cx.env.item_name(&r.result) } else { String::new() };
        if title1 == title2 {
            title2 = String::new();
        }
    }
    let height = kind.height();
    let (rx, ry) = (PAGE_WIDTH / 2 - 49, 4);
    if let Some(r) = &r1 {
        draw_recipe(p, r, &title1, rx, ry);
        if let Some(r2) = &r2 {
            draw_recipe(p, r2, &title2, rx, ry + height - if title2.is_empty() { 10 } else { 0 });
        }
    }
    let text_height = ry + height * if r2.is_none() { 1 } else { 2 } - if title2.is_empty() { 23 } else { 13 };
    if text_height + 10 < PAGE_HEIGHT {
        p.text(&page_text_of(j), text_height);
    }
}

fn draw_recipe(p: &mut PageOut<'_>, r: &RecipeView, title: &str, rx: i32, ry: i32) {
    let cx = p.cx;
    match r.kind {
        RecipeKind::Crafting => {
            p.blit(Tex::Crafting, rx - 2, ry - 2, 0, 0, 100, 62);
            if !r.shaped {
                let (ix, iy) = (rx + 62, ry + 2);
                p.blit(Tex::Crafting, ix, iy, 0, 64, 11, 11);
                p.tip(ix, iy, 11, 11, vec![cx.tr("patchouli.gui.lexicon.shapeless", "Shapeless")]);
            }
            p.centered(title, PAGE_WIDTH / 2, ry - 10);
            p.item(vec![r.result.clone()], rx + 79, ry + 22);
            let wrap = if r.shaped { r.width.max(1) } else { 3 };
            for (i, ing) in r.ingredients.iter().enumerate() {
                p.item(ing.clone(), rx + (i % wrap) as i32 * 19 + 3, ry + (i / wrap) as i32 * 19 + 3);
            }
            p.item(vec![Stack::new(r.kind.symbol(), 1)], rx + 79, ry + 41);
        }
        RecipeKind::Smithing => {
            p.blit(Tex::Crafting, rx, ry, 11, 135, 96, 43);
            p.centered(title, PAGE_WIDTH / 2, ry - 10);
            let get = |i: usize| r.ingredients.get(i).cloned().unwrap_or_default();
            p.item(get(0), rx + 4, ry + 4);
            p.item(get(1), rx + 4, ry + 23);
            p.item(get(2), rx + 40, ry + 4);
            p.item(vec![Stack::new(r.kind.symbol(), 1)], rx + 40, ry + 20);
            p.item(vec![r.result.clone()], rx + 76, ry + 13);
        }
        _ => {
            p.blit(Tex::Crafting, rx, ry, 11, 71, 96, 24);
            p.centered(title, PAGE_WIDTH / 2, ry - 10);
            p.item(r.ingredients.first().cloned().unwrap_or_default(), rx + 4, ry + 4);
            p.item(vec![Stack::new(r.kind.symbol(), 1)], rx + 40, ry + 4);
            p.item(vec![r.result.clone()], rx + 76, ry + 4);
        }
    }
}

fn template_page(p: &mut PageOut<'_>, entry: &Entry, page: &Page) {
    let cx = p.cx;
    let comps = match compile_template_page(cx.env, cx.contents(), cx.info(), page) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("{}: {e}", entry.id);
            p.text_at(&format!("$(#d00){}", msg.replace('$', "")), 0, 0, PAGE_WIDTH, TEXT_LINE_HEIGHT, 0xDD0000);
            return;
        }
    };
    for c in comps {
        if !c.visible(cx.env) {
            continue;
        }
        let (x, y) = (c.x, c.y);
        match c.kind {
            CompKind::Text { text, color, max_width, line_height } => {
                p.text_at(&text, x, y, max_width, line_height, color.unwrap_or(cx.info().text_color));
            }
            CompKind::Item { items, framed, .. } => {
                if items.is_empty() {
                    continue;
                }
                if framed {
                    p.blit(Tex::Crafting, x - 5, y - 5, 20, 102, 26, 26);
                }
                p.item(items, x, y);
            }
            CompKind::Image { image, u, v, width, height, tw, th, scale } => {
                if scale == 0.0 || image.is_empty() {
                    continue;
                }
                p.out.push(Elem::Image {
                    texture: qualify("minecraft", &image),
                    x: p.left + x,
                    y: p.top + y,
                    w: (width as f32 * scale).round() as i32,
                    h: (height as f32 * scale).round() as i32,
                    u,
                    v,
                    sw: width,
                    sh: height,
                    tw,
                    th,
                });
            }
            CompKind::Header { text, color, centered, scale } => {
                let t = p.i18n(&text);
                let color = color.unwrap_or(cx.info().header_color);
                let w = cx.width(&t);
                let sx = if centered { x - (w as f32 * scale / 2.0) as i32 } else { x };
                p.out.push(Elem::Str { text: t, x: p.left + sx, y: p.top + y, color, scale, uniform: false, shadow: false });
            }
            CompKind::Separator => p.separator(x, y),
            CompKind::Frame => p.blit(Tex::Book, x, y, 405, 149, 106, 106),
            CompKind::Entity { id, render_size, rotate, default_rotation } => {
                p.out.push(Elem::Entity { id, x: p.left + x, y: p.top + y, render_size, extra_offset: 0.0, rotate, default_rotation });
            }
            CompKind::Tooltip { lines, width, height } => p.tip(x, y, width, height, lines),
            CompKind::Unknown(_) => {}
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Navigation (GuiBook.displayLexiconGui, back, changePage, bookmarks)
// ------------------------------------------------------------------------------------------------

/// `GuiBookEntry.canBeOpened` and `GuiBookCategory.canBeOpened`.
fn can_open(env: &dyn Env, book: &Book, nav: &Nav, w: &Where) -> bool {
    match &w.gui {
        Gui::Entry(id) => book.contents.entries.contains_key(id) && !book.contents.entry_locked(env, &book.info, id) && *w != nav.cur,
        Gui::Category(id) => book.contents.categories.contains_key(id) && !book.contents.category_locked(env, &book.info, id) && *w != nav.cur,
        _ => true,
    }
}

/// `BookContents.openLexiconGui(gui, push)`; true when it was opened.
pub fn open_where(env: &dyn Env, book: &Book, persist: &mut Persist, nav: &mut Nav, w: Where, push: bool) -> bool {
    if !can_open(env, book, nav, &w) {
        return false;
    }
    if push && nav.cur != w {
        nav.stack.push(nav.cur.clone());
    }
    nav.cur = w.clone();
    nav.search.clear();
    nav.image_index.clear();
    first_opened(book, persist, &w);
    true
}

/// `GuiBookEntry.onFirstOpened`: the entry is read, and the first of the history.
fn first_opened(book: &Book, persist: &mut Persist, w: &Where) {
    let Gui::Entry(id) = &w.gui else { return };
    let data = persist.data(&book.info.id);
    let mut dirty = false;
    if !data.viewed.contains(id) {
        data.viewed.push(id.clone());
        dirty = true;
    }
    let index = data.history.iter().position(|h| h == id);
    if index != Some(0) {
        if let Some(i) = index {
            data.history.remove(i);
        }
        data.history.insert(0, id.clone());
        data.history.truncate(ENTRIES_PER_PAGE);
        dirty = true;
    }
    if dirty {
        persist.dirty = true;
    }
}

/// `BookContents.setTopEntry` and `displayBookGui`: the book opens at an entry. `None` keeps the last screen.
pub fn open_at_entry(env: &dyn Env, book: &Book, persist: &mut Persist, nav: &mut Nav, entry: &str, page: usize) {
    // checkValidCurrentEntry
    check_valid(env, book, nav);
    if !book.contents.entries.contains_key(entry) {
        return;
    }
    if book.contents.entry_locked(env, &book.info, entry) {
        return;
    }
    let spread = page / 2;
    let target = Where::entry(entry, spread);
    if nav.cur == target {
        return;
    }
    nav.stack.push(nav.cur.clone());
    nav.cur = target.clone();
    nav.search.clear();
    first_opened(book, persist, &target);
}

/// `BookContents.checkValidCurrentEntry`: a screen that cannot be opened any more (a locked entry) is forgotten.
pub fn check_valid(env: &dyn Env, book: &Book, nav: &mut Nav) {
    if !can_open_current(env, book, nav) {
        *nav = Nav::default();
    }
}

fn can_open_current(env: &dyn Env, book: &Book, nav: &Nav) -> bool {
    match &nav.cur.gui {
        Gui::Entry(id) => book.contents.entries.contains_key(id) && !book.contents.entry_locked(env, &book.info, id),
        Gui::Category(id) => book.contents.categories.contains_key(id) && !book.contents.category_locked(env, &book.info, id),
        _ => true,
    }
}

/// `GuiBook.back`.
pub fn back(env: &dyn Env, book: &Book, persist: &mut Persist, nav: &mut Nav, shift: bool) -> bool {
    if nav.stack.is_empty() {
        return false;
    }
    if shift {
        nav.cur = Where::landing();
        nav.stack.clear();
    } else if let Some(prev) = nav.stack.pop() {
        // displayLexiconGui(guiStack.pop(), false)
        if can_open(env, book, nav, &prev) {
            nav.cur = prev.clone();
            first_opened(book, persist, &prev);
        }
    }
    nav.search.clear();
    nav.image_index.clear();
    true
}

/// `GuiBook.changePage`: true when the spread changed.
pub fn change_page(view: &View, nav: &mut Nav, left: bool) -> bool {
    let can = if left { nav.cur.spread > 0 } else { nav.cur.spread + 1 < view.max_spreads };
    if !can {
        return false;
    }
    if left {
        nav.cur.spread -= 1;
    } else {
        nav.cur.spread += 1;
    }
    nav.image_index.clear();
    true
}

/// What a click on `cmd` does to the navigation and the reading state.
pub fn apply_cmd(env: &dyn Env, book: &Book, persist: &mut Persist, nav: &mut Nav, view: &View, cmd: &Cmd, shift: bool) -> Vec<Effect> {
    let mut fx = Vec::new();
    match cmd {
        Cmd::Open(w) => {
            if open_where(env, book, persist, nav, w.clone(), true) {
                fx.push(Effect::Flip);
            }
        }
        Cmd::Entry(id) => {
            let w = Where::entry(id, 0);
            let mut handled = false;
            if shift {
                let data = persist.data(&book.info.id);
                let already = data.bookmarks.iter().any(|(e, s)| e == id && *s == 0);
                if already {
                    data.bookmarks.retain(|(e, s)| !(e == id && *s == 0));
                    handled = true;
                } else if data.bookmarks.len() < MAX_BOOKMARKS {
                    data.bookmarks.push((id.clone(), 0));
                    handled = true;
                }
                persist.dirty |= handled;
            }
            if !handled && open_where(env, book, persist, nav, w, true) {
                fx.push(Effect::Flip);
            }
        }
        Cmd::Back => {
            if back(env, book, persist, nav, shift) {
                fx.push(Effect::Flip);
            }
        }
        Cmd::Page(left) => {
            if change_page(view, nav, *left) {
                fx.push(Effect::Flip);
            }
        }
        Cmd::Bookmark(index) => {
            let existing = index.and_then(|i| persist.get(&book.info.id).and_then(|d| d.bookmarks.get(i).cloned()));
            match existing {
                Some((entry, spread)) if book.contents.entries.contains_key(&entry) => {
                    if shift {
                        if let Some(i) = index {
                            persist.data(&book.info.id).bookmarks.remove(*i);
                            persist.dirty = true;
                        }
                    } else if open_where(env, book, persist, nav, Where::entry(&entry, spread), true) {
                        fx.push(Effect::Flip);
                    }
                }
                _ => {
                    // bookmarkThis (only the entries have it)
                    if let Gui::Entry(id) = nav.cur.gui.clone() {
                        persist.data(&book.info.id).bookmarks.push((id, nav.cur.spread));
                        persist.dirty = true;
                    }
                }
            }
            fx.push(Effect::Flip);
        }
        Cmd::MarkRead => {
            let main = nav.stack.is_empty();
            let ids: Vec<String> = book.contents.entries.keys().cloned().collect();
            let cat_filter: Option<String> = if let Gui::Category(c) = &nav.cur.gui { Some(c.clone()) } else { None };
            for id in ids {
                let in_scope = main || cat_filter.as_ref().is_none_or(|c| in_category(book, &id, c));
                if in_scope
                    && !book.contents.entry_locked(env, &book.info, &id)
                    && book.contents.entry_state(env, persist, &book.info, &id) == ReadState::Unread
                {
                    persist.data(&book.info.id).viewed.push(id);
                    persist.dirty = true;
                }
            }
            fx.push(Effect::Flip);
        }
        Cmd::ToggleQuest(entry) => {
            let data = persist.data(&book.info.id);
            if let Some(i) = data.completed_quests.iter().position(|q| q == entry) {
                data.completed_quests.remove(i);
            } else {
                data.completed_quests.push(entry.clone());
            }
            persist.dirty = true;
        }
        Cmd::Image { page, left } => {
            let i = nav.image_index.entry(*page).or_insert(0);
            if *left {
                *i = i.saturating_sub(1);
            } else {
                *i += 1;
            }
            fx.push(Effect::Flip);
        }
        Cmd::OpenBook(_) => {}
        Cmd::Link(l) => match l {
            Link::Entry { id, spread } => {
                let _ = open_where(env, book, persist, nav, Where::entry(id, *spread), true);
                fx.push(Effect::Flip);
            }
            Link::Category(id) => {
                let _ = open_where(env, book, persist, nav, Where { gui: Gui::Category(id.clone()), spread: 0 }, true);
                fx.push(Effect::Flip);
            }
            Link::Url(u) => fx.push(Effect::OpenUrl(u.clone())),
            Link::Command(c) => fx.push(Effect::Command(c.clone())),
        },
    }
    if persist.dirty {
        fx.push(Effect::Save);
    }
    fx
}

/// The entry is in the category or in one of its children.
fn in_category(book: &Book, entry: &str, category: &str) -> bool {
    let Some(e) = book.contents.entries.get(entry) else { return false };
    let mut cur = Some(e.category.clone());
    let mut guard = 0;
    while let Some(c) = cur {
        if c == category {
            return true;
        }
        guard += 1;
        if guard > 16 {
            break;
        }
        cur = book.contents.categories.get(&c).and_then(|cat| cat.parent.clone());
    }
    false
}

/// A click that is not on a button: Shift on an item opens the entry that explains it. Returns the effect list.
pub fn open_for_stack(env: &dyn Env, book: &Book, persist: &mut Persist, nav: &mut Nav, item: &str) -> Vec<Effect> {
    let Some((entry, spread)) = book.contents.entry_for_stack(item).cloned() else { return Vec::new() };
    if matches!(&nav.cur.gui, Gui::Entry(e) if *e == entry) {
        return Vec::new();
    }
    let mut fx = Vec::new();
    if open_where(env, book, persist, nav, Where::entry(&entry, spread), true) {
        fx.push(Effect::Flip);
    }
    if persist.dirty {
        fx.push(Effect::Save);
    }
    fx
}

/// The search bar of the lists: the text changed.
pub fn set_search(env: &dyn Env, book: &Book, persist: &Persist, nav: &mut Nav, text: &str) {
    nav.search = text.to_owned();
    normalize(env, book, persist, nav);
}

// ------------------------------------------------------------------------------------------------
// The list of books (not in Patchouli: a guide book with no book in it opens this)
// ------------------------------------------------------------------------------------------------

/// A book in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryItem {
    pub id: String,
    pub title: String,
    pub entries: usize,
    pub icon: Icon,
}

/// The list of books as a book: the brown cover, a text on the left and the books on the right (11 on the first
/// spread, 26 on the next ones, as the lists of entries).
pub fn build_library(env: &dyn Env, items: &[LibraryItem], spread: usize) -> View {
    let info = BookInfo::from_json("patchouli", "library", &serde_json::json!({"name": "Patchouli", "use_resource_pack": true})).unwrap();
    let book = Book { info, contents: Contents::default(), lang: env.lang_code() };
    let persist = Persist::default();
    let cx = Cx::new(env, &book, &persist);
    let mut out: Vec<Elem> = Vec::new();
    out.push(Elem::Blit { tex: Tex::Book, x: 0, y: 0, w: FULL_WIDTH, h: FULL_HEIGHT, u: 0, v: 0, sw: FULL_WIDTH, sh: FULL_HEIGHT, tw: 512, th: 256, alpha: 1.0 });
    let mut view = View { title: "Patchouli".into(), ..View::default() };
    view.max_spreads = cx.list_max_spreads(items.len());
    let spread = spread.min(view.max_spreads - 1);
    blit(&mut out, Tex::Book, -8, 12, 0, 180, 140, 31);
    cx.string(&mut out, "Patchouli", 13, 16, book.info.nameplate_color);
    cx.string(&mut out, &format!("{} books", items.len()), 24, 24, book.info.nameplate_color);
    if spread == 0 {
        cx.text_translated(
            &mut out,
            "This guide book has no book of its own yet.$(br2)Pick one of the books of the installed mods; it opens like the guide book of that mod.$(br2)A book can be given with $(#490)/patchouli give <book>$().",
            LEFT_PAGE_X,
            TOP_PADDING + 25,
        );
        cx.centered(&mut out, "Books", RIGHT_PAGE_X + PAGE_WIDTH / 2, TOP_PADDING, book.info.header_color);
        separator(&mut out, RIGHT_PAGE_X, TOP_PADDING + 12);
    }
    let btn = |out: &mut Vec<Elem>, x: i32, y: i32, it: &LibraryItem| {
        out.push(Elem::EntryBtn {
            x,
            y,
            label: it.title.clone(),
            locked: false,
            priority: false,
            argb: 0xFF00_0000 | book.info.text_color,
            icon: it.icon.clone(),
            state: ReadState::Neutral,
            cmd: Cmd::OpenBook(it.id.clone()),
        });
    };
    if spread == 0 {
        for (i, it) in items.iter().take(ENTRIES_IN_FIRST_PAGE).enumerate() {
            btn(&mut out, RIGHT_PAGE_X, TOP_PADDING + 20 + i as i32 * 11, it);
        }
    } else {
        let start = list_start(spread);
        for (i, it) in items.iter().skip(start).take(ENTRIES_PER_PAGE).enumerate() {
            btn(&mut out, LEFT_PAGE_X, TOP_PADDING + i as i32 * 11, it);
        }
        for (i, it) in items.iter().skip(start + ENTRIES_PER_PAGE).take(ENTRIES_PER_PAGE).enumerate() {
            btn(&mut out, RIGHT_PAGE_X, TOP_PADDING + i as i32 * 11, it);
        }
    }
    for left in [true, false] {
        let visible = if left { spread > 0 } else { spread + 1 < view.max_spreads };
        if visible {
            out.push(Elem::Button {
                rect: [if left { -4 } else { FULL_WIDTH - 14 }, FULL_HEIGHT - 6, 18, 10],
                u: 272,
                v: if left { 10 } else { 0 },
                tip: vec![if left { cx.tr("patchouli.gui.lexicon.button.prev_page", "Previous") } else { cx.tr("patchouli.gui.lexicon.button.next_page", "Next") }],
                cmd: Cmd::Page(left),
            });
        }
    }
    view.elems = out;
    view
}
