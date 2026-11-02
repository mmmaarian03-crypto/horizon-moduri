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
    include!("enchdesc_engine.rs");
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
