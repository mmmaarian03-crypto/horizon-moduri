// Overflowing Bars inside Horizon's game (nightly 142388e1c). The bars themselves are drawn by the other half
// of the mod, a module of `horizon_client::hud_layout` (native/hud.rs), because the HUD's draw calls are made
// there. This half reads what only the game knows, once per client tick: whether the mod is on and its sheets
// are in the GUI atlas, the player's armor toughness and how far the chat has to move up.
use super::Game;
use std::cell::Cell;

#[allow(dead_code)]
mod engine {
    include!("overflowingbars_engine.rs");
}

pub const ID: &str = "overflowingbars";
pub const TITLE: &str = "Overflowing Bars 8.0.1";
pub const DESCRIPTION: &str = "Health, armor and armor toughness bars that stack their extra points in colour instead of in more rows, with a row count, the chat and the experience level moved clear of them. the pack's overflowingbars-client.toml.";

thread_local! {
    /// The settings folder was read into the HUD half.
    static CONFIGURED: Cell<bool> = const { Cell::new(false) };
}

impl Game<'_> {
    /// HEAD on `native_tick`.
    pub(super) fn overflowingbars_tick(&mut self) {
        if !self.native_window() {
            return;
        }
        use horizon_client::hud_layout::overflowingbars as hud;
        if !CONFIGURED.with(|c| c.replace(true)) {
            hud::configure(&|key| self.native_setting(ID, key));
        }
        let on = self.native_on(ID);
        // The sheets are textures of the mod's content, in the GUI atlas as `overflowingbars:block/...`; without
        // them the game's own bars stay.
        let sheets = hud::SHEETS.iter().all(|s| self.gui.atlas.sprites.contains_key(*s));
        let (toughness, chat_offset) = match self.player.as_mut() {
            Some(p) => {
                let toughness = p
                    .attribute("minecraft:armor_toughness")
                    .or_else(|_| p.attribute("armor_toughness"))
                    .unwrap_or(0.0)
                    .floor() as i32;
                let armor = p
                    .attribute("minecraft:armor")
                    .or_else(|_| p.attribute("armor"))
                    .unwrap_or(0.0) as i32;
                let max_health = p.attribute("minecraft:max_health").unwrap_or(20.0) as f32;
                let offset = hud::chat_offset_for(max_health, p.health.absorption(), armor, toughness);
                (toughness, offset)
            }
            None => (0, 0),
        };
        hud::set_facts(on, sheets, toughness, chat_offset);
    }
}
