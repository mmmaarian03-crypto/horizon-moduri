// Modonomicon inside Horizon's game (nightly 142388e1c). The rules are the engine below
// (src/lib.rs); this part reads the world through the kit (tools/kit, pasted by tools/hnmod.py bundle).
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    // Modonomicon for Horizon: the rules, without any game code (tools/new_mod.py skeleton).
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

}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "modonomicon";

// kit_blocks (tools/kit): block names, properties and tags as the finished mods read them
// (fallingtree, comforts, kleeslabs, respawn_anchor.rs on the pin). Pasted by `tools/hnmod.py bundle`
// in place of `include!("kit_blocks.rs");` inside the mod's runtime module `crate::game::<id>`.
// Use: `kit_blocks::name_at(self, pos)`, `kit_blocks::block_in_tag(self, "minecraft:logs", &name)`.
// Tag names are the 26.3 ones (`minecraft:logs`, `c:...`); 1.20 names (`forge:...`) do not exist.
#[allow(dead_code)]
mod kit_blocks {
    use crate::game::Game;
    use horizon_world::registry::BlockStateId;

    /// Inside the world height and in a loaded chunk.
    pub(super) fn loaded(g: &Game<'_>, p: [i32; 3]) -> bool {
        p[1] >= g.dim.min_y() && p[1] <= g.dim.max_y() && g.world.loaded(p[0], p[2])
    }
    /// The state at `p` (air outside the world, as `world.block` answers).
    pub(super) fn state_at(g: &Game<'_>, p: [i32; 3]) -> BlockStateId {
        g.world.block(p[0], p[1], p[2])
    }
    /// The block name of a state (`"minecraft:oak_log"`).
    pub(super) fn name_of(g: &Game<'_>, state: BlockStateId) -> std::sync::Arc<str> {
        g.res.facts().get(state).name.clone()
    }
    /// The block name at `p`; `None` when not loaded or outside the world.
    pub(super) fn name_at(g: &Game<'_>, p: [i32; 3]) -> Option<std::sync::Arc<str>> {
        loaded(g, p).then(|| name_of(g, state_at(g, p)))
    }
    /// A block state property (`facing`, `half`, `age`...).
    pub(super) fn prop(g: &Game<'_>, state: BlockStateId, key: &str) -> Option<String> {
        g.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    /// The same block with some properties changed; `None` if a value is not valid.
    pub(super) fn with_props(
        g: &Game<'_>,
        state: BlockStateId,
        props: &[(&str, &str)],
    ) -> Option<BlockStateId> {
        g.with_props(state, props)
    }
    /// `state.is(BlockTags.X)`: block tag membership by block name.
    pub(super) fn block_in_tag(g: &Game<'_>, tag: &str, block: &str) -> bool {
        g.res
            .items()
            .block_tags
            .get(tag)
            .is_some_and(|s| s.contains(block))
    }
    /// `stack.is(ItemTags.X)`: item tag membership by item name.
    pub(super) fn item_in_tag(g: &Game<'_>, tag: &str, item: &str) -> bool {
        g.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|s| s.contains(item))
    }
    /// The item in the hand that acts (`getItemInHand(hand)`, `Game::held`).
    pub(super) fn held_name(g: &Game<'_>) -> Option<String> {
        g.held().map(|s| s.item.to_string())
    }
    /// The held item is in an item tag (`minecraft:axes`, `minecraft:pickaxes`...).
    pub(super) fn held_in_tag(g: &Game<'_>, tag: &str) -> bool {
        g.held().is_some_and(|s| item_in_tag(g, tag, &s.item))
    }
    /// The player is sneaking (`isShiftKeyDown`), as fallingtree reads it.
    pub(super) fn sneaking(g: &Game<'_>) -> bool {
        g.player.as_ref().is_some_and(|p| p.shift_key_down)
    }
    /// A block of namespace `namespace` (`is_mod_block(name, "mcwwindows")`).
    pub(super) fn is_mod_block(name: &str, namespace: &str) -> bool {
        name.strip_prefix(namespace)
            .is_some_and(|rest| rest.starts_with(':'))
    }
}
// kit_notice (tools/kit): a line over the hotbar (action bar) for whoever acts, as fallingtree
// does: a LAN guest gets a `SystemChat { overlay: true }` packet, the host the game's own
// `message` line. Pasted by `tools/hnmod.py bundle` in place of `include!("kit_notice.rs");`.
// Use: `kit_notice::action_bar(self, "Tree too big".to_owned())`, `kit_notice::chat(self, ..)`.
#[allow(dead_code)]
mod kit_notice {
    use crate::game::Game;
    use horizon_protocol::packets::play::{Clientbound, SystemChat};

    fn send(g: &mut Game<'_>, text: String, overlay: bool) {
        if g.lan_acting.is_some() {
            g.guest_packets.push(Clientbound::SystemChat(SystemChat {
                content: horizon_protocol::text::literal(&text),
                overlay,
            }));
        } else {
            // The host's window has one line over the hotbar for both (see `chat`).
            g.message = Some((text, std::time::Instant::now()));
        }
    }
    /// `player.displayClientMessage(text, true)`: the action bar.
    pub(super) fn action_bar(g: &mut Game<'_>, text: String) {
        send(g, text, true);
    }
    /// `player.displayClientMessage(text, false)`. For a guest this is a chat line; the host
    /// gets the same action-bar line (this pin has no plain host-side chat append without
    /// going through `chat.rs` internals; see TRANSLATOR.md).
    pub(super) fn chat(g: &mut Game<'_>, text: String) {
        send(g, text, false);
    }
}
// kit_sound (tools/kit): sounds, level events and particles at a block. Pasted by
// `tools/hnmod.py bundle` in place of `include!("kit_sound.rs");`.
// - `audio.play` is captured and sent to LAN guests too (`Audio::play` vs `play_unshared`).
// - `level_event_at` queues `Level.levelEvent` (2001 = block break particles + sound, 1000s =
//   sounds); everyone around sees/hears it.
// - `particles.add` is this window only (guests do not see it).
#[allow(dead_code)]
mod kit_sound {
    use crate::game::Game;

    fn center(p: [i32; 3]) -> [f64; 3] {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]) + 0.5,
            f64::from(p[2]) + 0.5,
        ]
    }
    /// `level.playSound(null, pos, SoundEvents.X, SoundSource.BLOCKS, volume, pitch)`.
    pub(super) fn at_block(g: &mut Game<'_>, sound: &str, pos: [i32; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(center(pos)), volume, pitch);
    }
    /// A sound at an exact point (entities, items).
    pub(super) fn at(g: &mut Game<'_>, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(at), volume, pitch);
    }
    /// `level.levelEvent(id, pos, data)`; `2001` with the block state id = break effect.
    pub(super) fn level_event(g: &mut Game<'_>, id: i32, pos: [i32; 3], data: i32) {
        g.level_event_at(id, pos, data);
    }
    /// `level.addParticle(type, x, y, z, dx, dy, dz)` for this window only.
    pub(super) fn particle(
        g: &mut Game<'_>,
        ty: horizon_client::particles::ParticleType,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        let options = horizon_client::particles::ParticleOptions::simple(ty);
        g.particles.add(&options, at, velocity);
    }
}
// kit_state (tools/kit): per-world mod state in side files, the torchmaster pattern
// (`<world>/data/<MOD_ID>/<dimension>.txt`, written tmp + rename). Pasted by `tools/hnmod.py bundle` in
// place of `include!("kit_state.rs");`. The adapter must define `const MOD_ID: &str = "<id>";`.
//
// Wiring (both hooks are in HOOKS.md, verified by torchmaster):
//   HEAD `horizon_client::game::chat::Game<'_>::load_world_state`, bindings = []
//        -> helper calls `crate::game::Game::<id>_world_opened()` -> `STATE.with(|s| s.borrow_mut().clear())`
//   HEAD `horizon_client::game::Game<'_>::save_game_metadata`, bindings game = &mut *self
//        -> helper calls `game.<id>_save()` -> `PerWorld::save(..)`
// The state is loaded lazily on first use, keyed by (world folder, dimension), so it is right
// on the integrated server thread and on a dedicated LAN host alike; a client of a remote
// server (`remote.is_some()`) never reads or writes files.
//
//   thread_local! { static STATE: RefCell<kit_state::PerWorld<MyState>> = RefCell::new(kit_state::PerWorld::new()); }
//   STATE.with(|s| { let mut s = s.borrow_mut(); let st = s.level(self, MyState::load); st.x += 1; s.dirty(self); });
#[allow(dead_code)]
mod kit_state {
    use crate::game::Game;
    use std::path::PathBuf;

    /// `<world>/data/<MOD_ID>/`, or `None` when there is no local world (remote server, menu).
    pub(super) fn dir(g: &Game<'_>) -> Option<PathBuf> {
        if g.remote.is_some() || g.world_root.as_os_str().is_empty() {
            return None;
        }
        Some(g.world_root.join("data").join(super::MOD_ID))
    }
    /// The current dimension as a file name (`minecraft_overworld`).
    pub(super) fn dim_key(g: &Game<'_>) -> String {
        g.dim.id().replace([':', '/'], "_")
    }
    /// Reads `<world>/data/<MOD_ID>/<file>`.
    pub(super) fn read(g: &Game<'_>, file: &str) -> Option<String> {
        std::fs::read_to_string(dir(g)?.join(file)).ok()
    }
    /// Writes `<world>/data/<MOD_ID>/<file>` atomically; logs and returns false on failure.
    pub(super) fn write(g: &Game<'_>, file: &str, text: &str) -> bool {
        let Some(d) = dir(g) else { return false };
        let path = d.join(file);
        let tmp = d.join(format!("{file}.tmp"));
        let written = std::fs::create_dir_all(&d)
            .and_then(|()| std::fs::write(&tmp, text))
            .and_then(|()| std::fs::rename(&tmp, &path));
        if let Err(e) = &written {
            log::warn!("{}: {} not saved: {e}", super::MOD_ID, path.display());
        }
        written.is_ok()
    }

    struct Level<T> {
        root: PathBuf,
        dim: String,
        state: T,
        dirty: bool,
    }
    /// One `T` per (world folder, dimension), loaded from `<dim>.txt` on first use.
    pub(super) struct PerWorld<T> {
        levels: Vec<Level<T>>,
    }
    impl<T> PerWorld<T> {
        pub(super) const fn new() -> Self {
            Self { levels: Vec::new() }
        }
        fn index(&self, g: &Game<'_>) -> Option<usize> {
            let dim = dim_key(g);
            self.levels
                .iter()
                .position(|l| l.root == g.world_root && l.dim == dim)
        }
        /// The state of the current world and dimension; `load` gets the file text (if any).
        pub(super) fn level(&mut self, g: &Game<'_>, load: impl FnOnce(Option<String>) -> T) -> &mut T {
            let i = match self.index(g) {
                Some(i) => i,
                None => {
                    let dim = dim_key(g);
                    let text = read(g, &format!("{dim}.txt"));
                    self.levels.push(Level {
                        root: g.world_root.clone(),
                        dim,
                        state: load(text),
                        dirty: false,
                    });
                    self.levels.len() - 1
                }
            };
            &mut self.levels[i].state
        }
        /// Marks the current level's state for the next save.
        pub(super) fn dirty(&mut self, g: &Game<'_>) {
            if let Some(i) = self.index(g) {
                self.levels[i].dirty = true;
            }
        }
        /// Writes every dirty level of the current world (call from `save_game_metadata`).
        pub(super) fn save(&mut self, g: &Game<'_>, save: impl Fn(&T) -> String) {
            if dir(g).is_none() {
                return;
            }
            for l in self.levels.iter_mut().filter(|l| l.root == g.world_root && l.dirty) {
                if write(g, &format!("{}.txt", l.dim), &save(&l.state)) {
                    l.dirty = false;
                }
            }
        }
        /// Forgets everything (call when a world opens: `load_world_state`).
        pub(super) fn clear(&mut self) {
            self.levels.clear();
        }
    }
}

thread_local! {
    static STATE: std::cell::RefCell<kit_state::PerWorld<engine::State>> =
        const { std::cell::RefCell::new(kit_state::PerWorld::new()) };
}

/// The world as the engine sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
}
impl engine::World for Live<'_, '_> {
    fn block(&self, p: engine::Pos) -> Option<String> {
        kit_blocks::name_at(self.game, p).map(|n| n.to_string())
    }
    fn in_tag(&self, tag: &str, block: &str) -> bool {
        kit_blocks::block_in_tag(self.game, tag, block)
    }
}

impl Game<'_> {
    /// HEAD on `copper_golems::Game::use_golem_statue` (a block's use in `use_hand`, before
    /// placement; not reached when sneaking with an item). `true` = handled, the hand swings.
    pub(super) fn modonomicon_use(&mut self, pos: [i32; 3]) -> bool {
        // The original runs on the server: a remote server's world is its own.
        if self.remote.is_some() {
            return false;
        }
        match engine::on_use(&Live { game: self }, pos) {
            engine::Action::Pass => false,
            engine::Action::Notice(text) => {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            *s.level(self, engine::State::load).uses.entry(pos).or_insert(0) += 1;
            s.dirty(self);
        });
                kit_sound::at_block(self, "minecraft:ui.button.click", pos, 0.3, 1.0);
                kit_notice::action_bar(self, text);
                true
            }
        }
    }

    /// HEAD `save_game_metadata`: the world is saved, so are this mod's side files.
    pub(super) fn modonomicon_save(&mut self) {
        let game: &Game<'_> = self;
        STATE.with(|s| s.borrow_mut().save(game, engine::State::save));
    }
    /// HEAD `load_world_state` (no bindings): a world opens; its state is read again on first use.
    pub(super) fn modonomicon_world_opened() {
        STATE.with(|s| s.borrow_mut().clear());
    }
}
