// Modonomicon for Horizon: the rules, without any game code (tools/nou_mod.py skeleton).
// Written from the original mod's behaviour and the pack's config; cite the source of every value.
// This file is pasted into the game as `crate::game::modonomicon::engine` (tools/hnmod.py bundle) and is also the
// `horizon_modonomicon` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

/// The pack's values (its 1.20.1 config folder), written out by a generator script.
/// Keeps only what the port uses, with the source of each value.
#[allow(dead_code)]
pub mod config {
    // ---- modonomicon-client.toml
    /// Enable smooth zoom in book categories
    pub const QOL_ENABLE_SMOOTH_ZOOM: bool = true; // modonomicon-client.toml:5 [qol] enableSmoothZoom
    /// Enable keeping the last open page stored when closing an entry. Regardless of this setting it will be stored when closing the entire book with Esc.
    pub const QOL_STORE_LAST_OPEN_PAGE_WHEN_CLOSING_ENTRY: bool = false; // modonomicon-client.toml:7 [qol] storeLastOpenPageWhenClosingEntry
    /// If your locale is not supported by the default Modonomicon font, indicated by the book just rendering blocky shapes instead of characters, add your locale to this list to fall back
    pub const QOL_FONT_FALLBACK_LOCALES: &[&str] = &["zh_cn", "ja_jp", "ko_kr"]; // modonomicon-client.toml:9 [qol] fontFallbackLocales
}

/// What the rules need to know about the world. The adapter (native/horizon.rs) implements it on
/// the game with kit_blocks; tests implement it on a map.
pub trait World {
    /// The block name at `p` (`"minecraft:stone"`); `None` when not loaded / outside the world.
    fn block(&self, p: Pos) -> Option<String>;
    /// `state.is(BlockTags.X)` by block name (26.3 tag names: `minecraft:logs`, `c:ores`...).
    fn in_tag(&self, tag: &str, block: &str) -> bool;
}

/// What a right-click on a block does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Not ours: the game goes on with its own use.
    Pass,
    /// Handled; show this line over the hotbar.
    Notice(String),
}

/// Right-click on the block at `pos` (`Block.use` / `PlayerInteractEvent.RightClickBlock`).
pub fn on_use(world: &impl World, pos: Pos) -> Action {
    match world.block(pos) {
        Some(name) if name.starts_with("modonomicon:") => Action::Notice(format!("{name} at {pos:?}")),
        _ => Action::Pass,
    }
}

/// Per-world state saved in `<world>/data/modonomicon/<dimension>.txt` (kit_state). One line per entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub uses: BTreeMap<Pos, u32>,
}
impl State {
    pub fn load(text: Option<String>) -> Self {
        let mut s = State::default();
        for line in text.unwrap_or_default().lines() {
            let v: Vec<i64> = line.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if let [x, y, z, n] = v[..] {
                s.uses.insert([x as i32, y as i32, z as i32], n as u32);
            }
        }
        s
    }
    pub fn save(&self) -> String {
        let mut out = String::new();
        for (p, n) in &self.uses {
            out.push_str(&format!("{} {} {} {}\n", p[0], p[1], p[2], n));
        }
        out
    }
}
