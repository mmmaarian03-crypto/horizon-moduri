// Enchantment Descriptions inside Horizon's game (nightly 142388e1c). The rules are the engine below
// (src/lib.rs); this part reads the stack, the language and the cursor, and edits the tooltip lines.
//
// The original adds the lines in `ItemTooltipEvent` (the end of `ItemStack.getTooltipLines`). Horizon's
// tooltip builder is `stations::Game::stack_tooltip_lines`, which has macros, so the hook is RETURN on the
// small plain `stack_tooltip` that every tooltip goes through (the inventory, the recipe book, other mods).
use super::{Game, ItemStack};
use horizon_client::text::Text;
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    // Enchantment Descriptions (Darkhax, 1.20.1 build 17.1.21) for Horizon: the rules, without any game code.
// Source of every rule: surse/enchdesc (branch 1.20.1) `EnchDescCommon.onItemTooltip`, `DescriptionManager`,
// `ConfigSchema`; the values are the pack's `config/enchdesc.json`.
// This file is pasted into the game as `crate::game::enchdesc::engine` (tools/hnmod.py bundle) and is also the
// `horizon_enchdesc` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).

/// the pack's file `enchdesc.json`, as Rust constants (the defaults of the original as well).
pub mod config {
    pub const ENABLE_MOD: bool = true; // enchdesc.json enableMod
    pub const ONLY_DISPLAY_ON_BOOKS: bool = false; // enchdesc.json onlyDisplayOnBooks
    pub const ONLY_DISPLAY_IN_ENCHANTING_TABLE: bool = false; // enchdesc.json onlyDisplayInEnchantingTable
    pub const REQUIRE_KEYBIND_PRESS: bool = false; // enchdesc.json requireKeybindPress
    pub const INDENT_SIZE: i64 = 0; // enchdesc.json indentSize
    pub const STYLE_COLOR: &str = "dark_gray"; // enchdesc.json style.color
}

/// The key of the hint line shown while the Shift key is required and not held
/// (`Component.translatable("enchdesc.activate.message")`).
pub const ACTIVATE_KEY: &str = "enchdesc.activate.message";
/// The hint is always dark gray (`withStyle(ChatFormatting.DARK_GRAY)`).
pub const HINT_COLOR: u32 = 0x55_5555;

/// `ChatFormatting` colours (name, RGB), the names `Style.FORMATTING_CODEC` accepts for `color`.
pub const NAMED_COLORS: [(&str, u32); 16] = [
    ("black", 0x00_0000),
    ("dark_blue", 0x00_00AA),
    ("dark_green", 0x00_AA00),
    ("dark_aqua", 0x00_AAAA),
    ("dark_red", 0xAA_0000),
    ("dark_purple", 0xAA_00AA),
    ("gold", 0xFF_AA00),
    ("gray", 0xAA_AAAA),
    ("dark_gray", 0x55_5555),
    ("blue", 0x55_55FF),
    ("green", 0x55_FF55),
    ("aqua", 0x55_FFFF),
    ("red", 0xFF_5555),
    ("light_purple", 0xFF_55FF),
    ("yellow", 0xFF_FF55),
    ("white", 0xFF_FFFF),
];

/// `TextColor.parseColor`: a colour name or `#RRGGBB`.
pub fn parse_color(text: &str) -> Option<u32> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix('#') {
        return (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok()).flatten();
    }
    NAMED_COLORS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(text))
        .map(|(_, rgb)| *rgb)
}

/// The `style` of the config: the formatting of every description line (`Style` of the original).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub color: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    pub obfuscated: bool,
}

/// `ConfigSchema`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub enable_mod: bool,
    pub only_display_on_books: bool,
    pub only_display_in_enchanting_table: bool,
    pub require_keybind_press: bool,
    pub indent_size: usize,
    pub style: Style,
}

impl Config {
    /// the pack's values (`enchdesc.json`).
    pub fn tuned() -> Self {
        Self {
            enable_mod: config::ENABLE_MOD,
            only_display_on_books: config::ONLY_DISPLAY_ON_BOOKS,
            only_display_in_enchanting_table: config::ONLY_DISPLAY_IN_ENCHANTING_TABLE,
            require_keybind_press: config::REQUIRE_KEYBIND_PRESS,
            indent_size: config::INDENT_SIZE.max(0) as usize,
            style: Style {
                color: parse_color(config::STYLE_COLOR),
                bold: false,
                italic: false,
                underlined: false,
                strikethrough: false,
                obfuscated: false,
            },
        }
    }

    /// the pack's values, then whatever the settings folder of the mod overrides (one file per key, named as
    /// in the original JSON: `enableMod`, `onlyDisplayOnBooks`, `onlyDisplayInEnchantingTable`,
    /// `requireKeybindPress`, `indentSize`, `style.color`, `style.bold`, `style.italic`, `style.underlined`,
    /// `style.strikethrough`, `style.obfuscated`). A value that does not parse keeps the pack's.
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
        flag("enableMod", &mut c.enable_mod);
        flag("onlyDisplayOnBooks", &mut c.only_display_on_books);
        flag("onlyDisplayInEnchantingTable", &mut c.only_display_in_enchanting_table);
        flag("requireKeybindPress", &mut c.require_keybind_press);
        if let Some(n) = get("indentSize").and_then(|v| v.trim().parse::<i64>().ok()) {
            c.indent_size = n.max(0) as usize;
        }
        if let Some(color) = get("style.color").and_then(|v| parse_color(&v)) {
            c.style.color = Some(color);
        }
        flag("style.bold", &mut c.style.bold);
        flag("style.italic", &mut c.style.italic);
        flag("style.underlined", &mut c.style.underlined);
        flag("style.strikethrough", &mut c.style.strikethrough);
        flag("style.obfuscated", &mut c.style.obfuscated);
        c
    }
}

/// The translations the rules ask for (`I18n.exists` / the translated text).
pub trait Lang {
    fn has(&self, key: &str) -> bool;
    fn get(&self, key: &str) -> Option<String>;
}

/// `DescriptionManager.get`: `<descriptionId>.desc`, or `<descriptionId>.description` when only that one exists.
pub fn description_key(description_id: &str, lang: &impl Lang) -> String {
    let desc = format!("{description_id}.desc");
    if !lang.has(&desc) {
        let long = format!("{description_id}.description");
        if lang.has(&long) {
            return long;
        }
    }
    desc
}

/// One enchantment of the stack, in the order of `EnchantmentHelper.getEnchantments`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ench {
    /// `enchantment.getFullname(level).getString()`: the tooltip line the description follows.
    pub full_name: String,
    /// `enchantment.getDescriptionId()`.
    pub description_id: String,
}

/// What the tooltip event knows about the stack and the screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// `stack.isEnchanted()`.
    pub enchanted: bool,
    /// `stack.getItem() instanceof EnchantedBookItem`.
    pub book: bool,
    /// `Minecraft.getInstance().screen instanceof EnchantmentScreen`.
    pub enchanting_table: bool,
    /// `Screen.hasShiftDown()`.
    pub shift: bool,
}

/// The tooltip after the edit: the old lines (by index), the descriptions and the hint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The line `lines[i]` of the tooltip as it was.
    Line(usize),
    /// A description line: the translation key it came from and its text.
    Description { key: String, text: String },
    /// The "hold shift" hint, at the end of the tooltip.
    Hint { text: String },
}

/// `onItemTooltip`: where the descriptions go. `lines` are the plain texts of the tooltip lines, `enchants`
/// the stack's enchantments (the stored ones for a book). `None` = leave the tooltip alone.
pub fn edit(
    cfg: &Config,
    ctx: &Context,
    lines: &[String],
    enchants: &[Ench],
    lang: &impl Lang,
) -> Option<Vec<Entry>> {
    // `this.config.enableMod && !stack.isEmpty() && stack.hasTag()` (an enchanted stack has a tag).
    if !cfg.enable_mod {
        return None;
    }
    if !((!cfg.only_display_on_books && ctx.enchanted) || ctx.book) {
        return None;
    }
    if cfg.only_display_in_enchanting_table && !ctx.enchanting_table {
        return None;
    }
    if enchants.is_empty() {
        return None;
    }
    let mut out: Vec<Entry> = (0..lines.len()).map(Entry::Line).collect();
    if cfg.require_keybind_press && !ctx.shift {
        out.push(Entry::Hint {
            text: lang.get(ACTIVATE_KEY).unwrap_or_else(|| ACTIVATE_KEY.to_owned()),
        });
        return Some(out);
    }
    for ench in enchants {
        // `for (Component line : tooltip) if (line.equals(fullName))`: the first line with that name.
        let at = out
            .iter()
            .position(|e| matches!(e, Entry::Line(i) if lines[*i] == ench.full_name));
        let Some(at) = at else { continue };
        let key = description_key(&ench.description_id, lang);
        // `Component.translatable(key)`: a missing translation shows the key.
        let mut text = lang.get(&key).unwrap_or_else(|| key.clone());
        if cfg.indent_size > 0 {
            text = format!("{}{}", " ".repeat(cfg.indent_size), text);
        }
        out.insert(at + 1, Entry::Description { key, text });
    }
    Some(out)
}

/// `ForgeHooksClient.gatherTooltipComponents`: the width the text is wrapped to when the tooltip fits on
/// neither side of the cursor (`mouse_x`), or `None` when it fits. `text_w` = the widest line.
pub fn wrap_limit(mouse_x: f32, screen_w: f32, text_w: f32) -> Option<f32> {
    let mut x = mouse_x + 12.0;
    if x + text_w + 4.0 > screen_w {
        x = mouse_x - 16.0 - text_w;
        if x < 4.0 {
            return Some(if mouse_x > screen_w / 2.0 {
                mouse_x - 12.0 - 8.0
            } else {
                screen_w - 16.0 - mouse_x
            });
        }
    }
    None
}

}

pub const ID: &str = "enchdesc";
pub const TITLE: &str = "EnchantmentDescriptions 17.1.21";
pub const DESCRIPTION: &str = "Adds a description under each enchantment in item and book tooltips. the pack's enchdesc.json.";

thread_local! {
    /// `ConfigSchema`, read once (the pack's values unless a file in the mod's settings folder says otherwise).
    static CONFIG: RefCell<Option<engine::Config>> = const { RefCell::new(None) };
}

/// The language the game shows (`I18n.exists` / `I18n.get`), with the pinned English map behind it.
struct GameLang<'a, 'g>(&'a Game<'g>);
impl engine::Lang for GameLang<'_, '_> {
    fn has(&self, key: &str) -> bool {
        self.0.pack.names.display_get(key).is_some()
    }
    fn get(&self, key: &str) -> Option<String> {
        self.0.pack.names.display_get(key)
    }
}

impl Game<'_> {
    fn enchdesc_config(&self) -> engine::Config {
        CONFIG.with(|c| {
            c.borrow_mut()
                .get_or_insert_with(|| engine::Config::read(|key| self.native_setting(ID, key)))
                .clone()
        })
    }

    /// RETURN on `stations::Game::stack_tooltip`: the item's tooltip lines with the descriptions in.
    pub(super) fn enchdesc_tooltip(
        &self,
        stack: &ItemStack,
        lines: Vec<(Text, u32)>,
    ) -> Vec<(Text, u32)> {
        if !self.native_on(ID) || lines.is_empty() {
            return lines;
        }
        let cfg = self.enchdesc_config();
        let book = &*stack.item == "minecraft:enchanted_book";
        // `EnchantmentHelper.getEnchantments`: a book's stored enchantments, else the item's own.
        let levels = if book {
            stack.stored_enchantments()
        } else {
            stack.enchantments()
        };
        if levels.is_empty() {
            return lines;
        }
        let registry = &self.res.stations().enchantments;
        let enchants: Vec<engine::Ench> = levels
            .iter()
            .map(|(id, level)| engine::Ench {
                full_name: registry.full_name(id, *level).0,
                description_id: registry.get(id).map_or_else(
                    || {
                        let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
                        format!("enchantment.{ns}.{path}")
                    },
                    |e| e.description_key.clone(),
                ),
            })
            .collect();
        let plain: Vec<String> = lines.iter().map(|(t, _)| t.plain()).collect();
        let ctx = engine::Context {
            enchanted: !stack.enchantments().is_empty(),
            book,
            enchanting_table: self
                .station
                .as_ref()
                .is_some_and(|s| s.kind == super::stations::Kind::Enchanting),
            shift: self.has_shift_down(),
        };
        let Some(entries) = engine::edit(&cfg, &ctx, &plain, &enchants, &GameLang(self)) else {
            return lines;
        };
        // The description lines: the config's style on the text.
        let style = |text: &str| {
            use horizon_client::text::Style;
            let on = |v: bool| v.then_some(true);
            Text::literal(text).with_style(&Style {
                color: cfg.style.color,
                bold: on(cfg.style.bold),
                italic: on(cfg.style.italic),
                underlined: on(cfg.style.underlined),
                strikethrough: on(cfg.style.strikethrough),
                obfuscated: on(cfg.style.obfuscated),
                ..Style::default()
            })
        };
        // Horizon's tooltip does not wrap its lines: wrap the descriptions the way
        // `gatherTooltipComponents` wraps them when the tooltip fits on neither side of the cursor.
        let atlas = &self.gui.atlas;
        let width_of = |t: &Text| atlas.font.width_text(&t.resolve_legacy());
        let mut text_w = lines.iter().map(|(t, _)| width_of(t)).fold(0.0f32, f32::max);
        for e in &entries {
            if let engine::Entry::Description { text, .. } = e {
                text_w = text_w.max(width_of(&style(text)));
            }
        }
        let limit = engine::wrap_limit(self.cursor.0, self.gui_size().0, text_w);
        let mut old: Vec<Option<(Text, u32)>> = lines.into_iter().map(Some).collect();
        let mut out = Vec::with_capacity(entries.len());
        for entry in entries {
            match entry {
                engine::Entry::Line(i) => {
                    if let Some(line) = old[i].take() {
                        out.push(line);
                    }
                }
                engine::Entry::Description { text, .. } => {
                    let t = style(&text);
                    match limit {
                        Some(w) => {
                            for (part, _) in atlas.font.split_lines(&t, w) {
                                out.push((part, u32::MAX));
                            }
                        }
                        None => out.push((t, u32::MAX)),
                    }
                }
                engine::Entry::Hint { text } => {
                    out.push((Text::literal(text).with_color(engine::HINT_COLOR), u32::MAX));
                }
            }
        }
        out
    }
}
