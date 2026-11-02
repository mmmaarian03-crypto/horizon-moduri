// Escape Rope inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the
// player tick that remembers the last place under the sky (`LastSkyCoordProcedure`), the rope's use
// (`EscapeRopeUseProcedure`) and the tooltip line. The item itself, its model and recipe are content
// (content/mods/escape_rope).
//
// Where it runs: everything the mod does is server side (the procedures run on the logical server). Since
// Horizon #1514 single player is an integrated server and the window is one of its guests, so:
// - the tick of every guest (single player's window included) runs in `tick_guests` on the server, and the
//   in-frame host's in `tick_blocks`;
// - the use runs inside the guest's turn (`as_guest`: `self.player`, the inventory and the player's data are
//   the guest's); the teleport reaches the guest's client through `leave_guest`, the action bar line through
//   `guest_packets` (kit_notice).
// A server's client (`remote`) does nothing but send the click.
#[allow(unused_imports)]
use super::{Game, Result, lan::LanHost};
use horizon_world::nbt::{JavaString, Tag};

#[allow(dead_code)]
mod engine {
    // Escape Rope for Horizon: the rules, without any game code.
// Written from the bytecode of `escape_rope-1.0.0.jar` (an MCreator mod, Academic Free License v3.0):
// `EscapeRopeUseProcedure`, `LastSkyCoordProcedure`, `EscapeRopeModVariables`, `EscapeRopeItem`. The mod
// has no config file. This file is pasted into the game as `crate::game::escape_rope::engine` and is also
// the `horizon_escape_rope` crate the tests use: plain Rust, no game types, no inner attributes.

pub type Pos = [i32; 3];

/// The pack has no config file for this mod.
pub mod config {}

pub const ITEM: &str = "escape_rope:escape_rope";
/// `EscapeRopeUseProcedure`: the lines over the hotbar.
pub const USED: &str = "You used the escape rope!";
pub const BLOCKED: &str = "You cannot use that here!";
/// `EscapeRopeItem.appendHoverText`.
pub const TOOLTIP: &str = "This handy rope will take you to the last place you could see the sky.";
/// `LevelReader.getSeaLevel()` (the default method; no dimension of the pack overrides it).
pub const SEA_LEVEL: i32 = 63;

/// The world as the sky tests read it.
pub trait Sky {
    /// `getHeight(Heightmap.Types.WORLD_SURFACE, x, z)`: one above the highest block that is not air.
    fn surface(&self, x: i32, z: i32) -> i32;
    /// `state.getLightBlock(level, pos) > 0 && !state.liquid()` for the block at `p`.
    fn blocks_light_not_liquid(&self, p: Pos) -> bool;
}

/// `LevelReader.canSeeSky(pos)`.
pub fn can_see_sky(world: &impl Sky, p: Pos) -> bool {
    p[1] >= world.surface(p[0], p[2])
}

/// `LevelReader.canSeeSkyFromBelowWater(pos)` (the `m_46861_` both procedures call): at or above the sea
/// level, `canSeeSky`; below it, the sea level's block of the column must see the sky and nothing that
/// stops light (water and lava excepted) may stand between it and `pos`.
pub fn can_see_sky_from_below_water(world: &impl Sky, p: Pos) -> bool {
    if p[1] >= SEA_LEVEL {
        return can_see_sky(world, p);
    }
    if !can_see_sky(world, [p[0], SEA_LEVEL, p[2]]) {
        return false;
    }
    let mut y = SEA_LEVEL - 1;
    while y > p[1] {
        if world.blocks_light_not_liquid([p[0], y, p[2]]) {
            return false;
        }
        y -= 1;
    }
    true
}

/// `BlockPos.containing(x, y, z)`.
pub fn containing(at: [f64; 3]) -> Pos {
    [at[0].floor() as i32, at[1].floor() as i32, at[2].floor() as i32]
}

/// `PlayerVariables.LastSkyX / LastSkyY / LastSkyZ`: the last place the player could see the sky. The
/// variables start at 0 and are kept through death (`clonePlayer` copies them whatever `isWasDeath`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Last {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Whether the player has been seen under the sky at all (not in the original: it tells a stored
    /// place from the 0, 0, 0 the variables start with).
    pub set: bool,
}
impl Last {
    /// One line per number, as `{:?}` writes a `f64` (exact round trip), then `set`.
    pub fn save(&self) -> String {
        format!("{:?}\n{:?}\n{:?}\n{}\n", self.x, self.y, self.z, self.set)
    }
    /// The inverse; anything unreadable is a player who has not seen the sky yet.
    pub fn load(text: Option<String>) -> Self {
        let text = text.unwrap_or_default();
        let mut lines = text.lines();
        let mut num = || lines.next().and_then(|l| l.trim().parse::<f64>().ok());
        match (num(), num(), num()) {
            (Some(x), Some(y), Some(z)) if x.is_finite() && y.is_finite() && z.is_finite() => {
                Last { x, y, z, set: true }
            }
            _ => Last::default(),
        }
    }
}

/// `LastSkyCoordProcedure` (`PlayerTickEvent`, END): when the player's block sees the sky, remember where
/// he stands (the exact position). `true` when the stored place changed.
pub fn track(world: &impl Sky, feet: [f64; 3], last: &mut Last) -> bool {
    if !can_see_sky_from_below_water(world, containing(feet)) {
        return false;
    }
    let now = Last { x: feet[0], y: feet[1], z: feet[2], set: true };
    let changed = *last != now;
    *last = now;
    changed
}

/// What using the rope does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rope {
    /// "You cannot use that here!": the player sees the sky. Nothing is used up.
    Blocked,
    /// The player has never been seen under the sky (the original would take him to 0, 0, 0, inside the
    /// rock): nothing happens and nothing is used up.
    NowhereToGo,
    /// Taken to the last place; the rope is used up (unless creative); "You used the escape rope!".
    Teleport { to: [f64; 3] },
}

/// `EscapeRopeUseProcedure.execute`.
pub fn use_rope(world: &impl Sky, feet: [f64; 3], last: &Last) -> Rope {
    if can_see_sky_from_below_water(world, containing(feet)) {
        Rope::Blocked
    } else if !last.set {
        Rope::NowhereToGo
    } else {
        Rope::Teleport { to: [last.x, last.y, last.z] }
    }
}

}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "escape_rope";

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
    /// going through `chat.rs` internals; see TRADUCATOR.md).
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

/// The key of the place in the player's data (`PlayerVariables` of the original, a capability saved with the
/// player): `{LastSkyX, LastSkyY, LastSkyZ}` as doubles. Kept in the player's own compound (the host's
/// `level.player_extra`, a guest's `saved`), so each player has his own place and it follows him through
/// dimensions and deaths.
const KEY: &str = "EscapeRopeVariables";
/// The place an older version kept for the in-frame host: `<world>/data/escape_rope/player.txt`. Read once,
/// when the host's data has no place yet.
const OLD_FILE: &str = "player.txt";
thread_local! {
    /// The world whose old file was already looked at.
    static OLD_TRIED: std::cell::RefCell<Option<std::path::PathBuf>> = const { std::cell::RefCell::new(None) };
}

fn double(t: Option<&Tag>) -> Option<f64> {
    match t? {
        Tag::Double(bits) => Some(f64::from_bits(*bits)),
        Tag::Float(bits) => Some(f64::from(f32::from_bits(*bits))),
        Tag::Int(v) => Some(f64::from(*v)),
        _ => None,
    }
}

/// The stored place in a player's compound (`None`: the player has none yet).
fn read_last(extra: &Option<Tag>) -> Option<engine::Last> {
    let Some(Tag::Compound(map)) = extra else { return None };
    let Some(v) = map.get(&JavaString::from(KEY)) else { return None };
    let (x, y, z) = (double(v.get("LastSkyX"))?, double(v.get("LastSkyY"))?, double(v.get("LastSkyZ"))?);
    (x.is_finite() && y.is_finite() && z.is_finite()).then_some(engine::Last { x, y, z, set: true })
}

/// Writes the place into a player's compound (the other keys stay).
fn write_last(extra: &mut Option<Tag>, last: &engine::Last) {
    let e = extra.get_or_insert_with(|| Tag::Compound(Default::default()));
    let Tag::Compound(map) = e else { return };
    map.insert(
        JavaString::from(KEY),
        Tag::compound([
            ("LastSkyX", Tag::Double(last.x.to_bits())),
            ("LastSkyY", Tag::Double(last.y.to_bits())),
            ("LastSkyZ", Tag::Double(last.z.to_bits())),
        ]),
    );
}

/// The world as the sky tests read it.
struct Live<'a, 'g>(&'a Game<'g>);
impl engine::Sky for Live<'_, '_> {
    fn surface(&self, x: i32, z: i32) -> i32 {
        // getHeight(WORLD_SURFACE, x, z)
        self.0.world_surface(x, z)
    }
    fn blocks_light_not_liquid(&self, p: engine::Pos) -> bool {
        // state.getLightBlock(level, pos) > 0 && !state.liquid(): water, lava and bubble columns are liquid.
        let facts = self.0.res.facts().get(self.0.world.block(p[0], p[1], p[2]));
        facts.dampening > 0
            && !matches!(
                &*facts.name,
                "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column"
            )
    }
}

impl Game<'_> {
    /// The place of the player whose data is in `level.player_extra` (the in-frame host, or the guest whose turn
    /// it is). The in-frame host's place of an older version (player.txt) is taken over once.
    fn er_last(&mut self) -> Option<engine::Last> {
        if let Some(last) = read_last(&self.level.player_extra) {
            return Some(last);
        }
        // Once per world: the file is not read again every tick of a player who has no place yet.
        let root = self.world_root.clone();
        let first = |root: std::path::PathBuf| OLD_TRIED.with(|t| t.borrow_mut().replace(root.clone()).as_ref() != Some(&root));
        if self.lan_acting.is_none() && kit_state::dir(self).is_some() && first(root) {
            let old = engine::Last::load(kit_state::read(self, OLD_FILE));
            if old.set {
                write_last(&mut self.level.player_extra, &old);
                return Some(old);
            }
        }
        None
    }

    /// HEAD on `tick_blocks` (once per game tick): `LastSkyCoordProcedure` for the in-frame host (the player
    /// tick, END). A server has no player of its own (`self.player` is `None` on the integrated server).
    pub(super) fn escape_rope_tick(&mut self) {
        if self.remote.is_some() || self.lan_acting.is_some() {
            return;
        }
        let Some(p) = self.player.as_ref().filter(|p| !p.health.dead_or_dying()) else {
            return;
        };
        let feet = [p.position.x, p.position.y, p.position.z];
        let mut last = self.er_last().unwrap_or_default();
        if engine::track(&Live(self), feet, &mut last) {
            write_last(&mut self.level.player_extra, &last);
        }
    }

    /// HEAD on `tick_guests` (the guests' tick on the server, single player's window included):
    /// `LastSkyCoordProcedure` for each living guest in this level. The guest's data is its `saved`
    /// compound outside its turn, so no turn is needed.
    pub(super) fn escape_rope_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        for i in 0..host.hub.guests.len() {
            let g = &host.hub.guests[i];
            if !(host.hub.here(g) && g.loaded && g.placed && !g.dead) {
                continue;
            }
            let feet = g.position;
            let mut last = read_last(&g.saved).unwrap_or_default();
            if engine::track(&Live(self), feet, &mut last) {
                write_last(&mut host.hub.guests[i].saved, &last);
            }
        }
    }

    /// HEAD on `throw_item` (an item used, in the air or against a block, after the block's own use):
    /// `EscapeRopeItem.use` -> `EscapeRopeUseProcedure`. The original returns PASS, so the click is
    /// not consumed (the helper answers `false` to the game, the other hand may still act).
    /// Runs on the in-frame host's game and on a server inside the guest's turn; a server's client only
    /// sends the click to its server.
    pub(super) fn escape_rope_use(&mut self, stack: &horizon_client::items::ItemStack) {
        if &*stack.item != engine::ITEM || self.remote.is_some() {
            return;
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let feet = [p.position.x, p.position.y, p.position.z];
        let last = self.er_last().unwrap_or_default();
        match engine::use_rope(&Live(self), feet, &last) {
            engine::Rope::Blocked | engine::Rope::NowhereToGo => {
                kit_notice::action_bar(self, engine::BLOCKED.to_owned());
            }
            engine::Rope::Teleport { to } => {
                // entity.teleportTo(x, y, z) and the connection's teleport(x, y, z, yRot, xRot): the
                // same dimension, the same facing. For a guest, `leave_guest` sees the body moved and
                // teleports its client (ServerPlayer.teleport).
                let v = horizon_core::geometry::Vec3::new(to[0], to[1], to[2]);
                if let Some(p) = self.player.as_mut() {
                    p.position = v;
                    p.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
                    p.ground.on_ground = false;
                }
                // The window's interpolation start is the in-frame host's own.
                if self.lan_acting.is_none() {
                    self.previous_position = v;
                }
                // itemstack.shrink(1) (the creative player's count is restored by the game).
                self.consume_held();
                kit_notice::action_bar(self, engine::USED.to_owned());
                // level.playSound(null, BlockPos.containing(x, y, z), escape_rope:item_use, PLAYERS, 1, 1)
                // where the player stood when he used it.
                let at = engine::containing(feet);
                kit_sound::at_block(self, "escape_rope:item_use", at, 1.0, 1.0);
            }
        }
    }

    /// HEAD on `load_world_state` (no bindings): a world opens. Nothing is kept between worlds (the places
    /// live in the players' data).
    pub(super) fn escape_rope_world_opened() {
        OLD_TRIED.with(|t| *t.borrow_mut() = None);
    }
}
