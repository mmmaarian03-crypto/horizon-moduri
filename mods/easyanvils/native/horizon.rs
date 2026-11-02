// Easy Anvils inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (ModAnvilMenu, ItemInteractionHandler, AnvilBlockEntity, AnvilRenderer of Easy Anvils 8.0.2). The
// cost rules themselves run inside the library (`horizon_client::stations::easyanvils`, a copy of the
// engine), so the menu engine, the guests and the screen all see them. Horizon's anvil is a
// workstation without a block entity, so the items an anvil holds are a table of the mod (a file per
// dimension in the world folder, like the Torchmaster port): the two input slots of an open anvil
// are that anvil's inventory, kept when the menu closes, spilled when the anvil breaks and drawn on
// top of the block.
//
// Where it runs (since Horizon #1514 single player's window is a client of its integrated server):
// - the anvils' contents live where the world is: the in-frame host's game, or a server. A server's guests
//   (single player's window included) use an anvil through the server's menu engine (`lan_stations`):
//   `easyanvils` in `game/lan_stations.rs` (mixins/easyanvils/lan_stations.rs) fills the guest's menu from
//   the anvil when it opens, copies it back on every change and keeps the items in the anvil when it closes;
// - one player at a time holds an anvil's slots (`engine::may_bind`); another one gets a plain anvil;
// - the items on top are the client's: a server sends each guest the anvils of his level on foundry's channel
//   (`easyanvils:anvils`, the world file's text, again whenever they change) and the client draws them.
use super::{Game, lan::LanHost};
#[allow(dead_code)]
mod engine {
    include!("easyanvils_engine.rs");
}
use engine::{Pos, Store, Wear};
use horizon_assets::item_model::DisplayContext;
use super::stations::Kind;
use horizon_client::items::ItemStack;
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

const CFG: engine::Config = engine::Config::MARIAN;
/// Server -> client: (dimension, the anvils of that level as the world file's text).
const ANVILS: super::foundry::Channel<(String, String)> = super::foundry::Channel::new("easyanvils:anvils");

struct LevelState {
    root: PathBuf,
    dim: &'static str,
    store: Store<ItemStack>,
    dirty: bool,
    /// Bumped on every change of `store`: the guests' clients get the anvils again.
    generation: u64,
}
impl LevelState {
    fn touch(&mut self) {
        self.dirty = true;
        self.generation += 1;
    }
}

/// A server guest's open anvil whose slots he holds.
struct GuestAnvil {
    root: PathBuf,
    dim: &'static str,
    pos: Pos,
    /// The guest's window of that menu.
    window: i32,
}

/// Who holds an anvil's slots.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Holder {
    Host,
    Guest(u32),
}

/// A server's client: the anvils of its level, as its server sent them.
struct Mirror {
    dim: String,
    store: Store<ItemStack>,
}

#[derive(Default)]
struct Runtime {
    levels: Vec<LevelState>,
    /// The anvil whose menu the in-frame host has open and mirrored.
    bound: Option<(PathBuf, &'static str, Pos)>,
    /// A server's guests at an anvil, by connection.
    guests: HashMap<u32, GuestAnvil>,
    /// What each guest's client last got: (dimension, generation).
    sent: HashMap<u32, (&'static str, u64)>,
    /// A server's client.
    mirror: Option<Mirror>,
}

thread_local! {
    static RT: RefCell<Runtime> = RefCell::new(Runtime::default());
    /// The last anvil take paid no levels (a free rename: nothing in the right slot to risk the
    /// anvil for, `onAnvilRepair` with an empty right item).
    static FREE_TAKE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// The runtime of this thread; a nested call (which no hook makes) gets `None`, not a panic.
fn with_rt<T>(f: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    RT.with(|rt| rt.try_borrow_mut().ok().map(|mut rt| f(&mut rt)))
}

fn file_of(root: &Path, dim: &str) -> PathBuf {
    root.join("data")
        .join("easyanvils")
        .join(format!("{}.txt", dim.replace([':', '/'], "_")))
}
fn encode(stack: &ItemStack) -> String {
    horizon_client::item_data::canon_stack(Some(stack))
}
fn decode(text: &str) -> Option<ItemStack> {
    horizon_client::item_data::parse_stack(text).ok().flatten()
}

impl Runtime {
    fn level(&mut self, game: &Game<'_>) -> &mut LevelState {
        self.level_at(&game.world_root, game.dim.id())
    }
    fn level_at(&mut self, root: &Path, dim: &'static str) -> &mut LevelState {
        let index = match self.levels.iter().position(|l| l.dim == dim && l.root == root) {
            Some(i) => i,
            None => {
                let store = if root.as_os_str().is_empty() {
                    Store::default()
                } else {
                    std::fs::read_to_string(file_of(root, dim))
                        .map(|t| engine::load(&t, decode))
                        .unwrap_or_default()
                };
                self.levels.push(LevelState {
                    root: root.to_path_buf(),
                    dim,
                    store,
                    dirty: false,
                    generation: 1,
                });
                self.levels.len() - 1
            }
        };
        &mut self.levels[index]
    }
    /// Who holds which anvil of a level.
    fn holders(&self, root: &Path, dim: &str) -> Vec<(Pos, Holder)> {
        let host = self
            .bound
            .iter()
            .filter(|(r, d, _)| r == root && *d == dim)
            .map(|(_, _, p)| (*p, Holder::Host));
        let guests = self
            .guests
            .iter()
            .filter(|(_, t)| t.root == root && t.dim == dim)
            .map(|(c, t)| (t.pos, Holder::Guest(*c)));
        host.chain(guests).collect()
    }
}

impl Game<'_> {
    fn easyanvils_block(&self, p: Pos) -> std::sync::Arc<str> {
        let state = self.world.block(p[0], p[1], p[2]);
        self.res.facts().get(state).name.clone()
    }

    /// The open anvil menu of the in-frame host's own game: its position. A server's guest uses the server's
    /// menu engine (`lan_stations`): inside his turn `self.station` is not his menu (game/lan_stations.rs
    /// handles it); a server's client has no anvils of its own.
    fn easyanvils_open_anvil(&self) -> Option<Pos> {
        if self.remote.is_some() || self.lan_acting.is_some() {
            return None;
        }
        self.station
            .as_ref()
            .filter(|s| s.kind == Kind::Anvil)
            .map(|s| s.pos)
    }

    /// `open_station` returned: the menu of an anvil starts with the anvil's items
    /// (`ModAnvilMenu` is built over the block entity); a new anvil starts empty.
    /// A guest's use opens the station inside his turn, but his menu is made after it (`lan_open_station`):
    /// that one is filled in game/lan_stations.rs. An anvil a guest holds is a plain anvil for the host.
    pub(super) fn easyanvils_opened(&mut self, pos: [i32; 3], name: &str, opened: bool) {
        if !opened || self.remote.is_some() || self.lan_acting.is_some() || !engine::is_anvil(name) {
            return;
        }
        let empty = self.station.as_ref().is_none_or(|s| s.inputs.iter().all(Option::is_none));
        let Some(items) = with_rt(|rt| {
            let (root, dim) = (self.world_root.clone(), self.dim.id());
            if !engine::may_bind(&rt.holders(&root, dim), pos, &Holder::Host, empty) {
                rt.bound = None;
                return None;
            }
            rt.bound = Some((root, dim, pos));
            Some(rt.level(self).store.open(pos))
        })
        .flatten() else {
            return;
        };
        if let Some(s) = self.station.as_mut() {
            for (i, item) in items.into_iter().enumerate() {
                if let Some(cell) = s.inputs.get_mut(i) {
                    *cell = item;
                }
            }
        }
        // the result for the items that are already there
        self.station_changed();
    }

    /// The inputs go into the anvil's contents and out of the player's hands.
    fn easyanvils_stash(&mut self, pos: Pos) {
        let was_bound = with_rt(|rt| rt.bound.take()).flatten().is_some();
        if !was_bound {
            return;
        }
        let Some(s) = self.station.as_mut() else {
            return;
        };
        let items: Vec<Option<ItemStack>> = s.inputs.iter_mut().map(Option::take).collect();
        with_rt(|rt| {
            let ls = rt.level(self);
            ls.store.mirror(pos, &items);
            ls.touch();
        });
    }

    /// `close_station` first: the two inputs stay in the anvil instead of going back.
    pub(super) fn easyanvils_close(&mut self) {
        if let Some(pos) = self.easyanvils_open_anvil() {
            self.easyanvils_stash(pos);
        }
    }

    /// Every game tick: the open anvil's inputs are copied to it (a crash loses nothing), and the
    /// contents of anvils whose block is gone are spilled (`AnvilBlock.onRemove`).
    pub(super) fn easyanvils_tick(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        if let Some(pos) = self.easyanvils_open_anvil() {
            let bound = with_rt(|rt| {
                rt.bound.as_ref().is_some_and(|b| b.2 == pos && b.0 == self.world_root)
            })
            .unwrap_or(false);
            if bound {
                if engine::is_anvil(&self.easyanvils_block(pos)) {
                    let inputs: Vec<Option<ItemStack>> = self
                        .station
                        .as_ref()
                        .map(|s| s.inputs.clone())
                        .unwrap_or_default();
                    with_rt(|rt| {
                        let ls = rt.level(self);
                        if ls.store.mirror(pos, &inputs) {
                            ls.touch();
                        }
                    });
                } else {
                    // the block is gone: its items stay in the contents, which are spilled below
                    self.easyanvils_stash(pos);
                }
            }
        } else {
            with_rt(|rt| rt.bound = None);
        }
        // An anvil a guest holds is judged with his menu, in `tick_guests` (no item is spilled twice).
        let mut gone: Vec<(Pos, Vec<Option<ItemStack>>)> = Vec::new();
        with_rt(|rt| {
            let (root, dim) = (self.world_root.clone(), self.dim.id());
            let held: Vec<Pos> = rt
                .holders(&root, dim)
                .into_iter()
                .filter(|(_, h)| matches!(h, Holder::Guest(_)))
                .map(|(p, _)| p)
                .collect();
            let ls = rt.level(self);
            gone = ls.store.take_gone(|p| {
                self.world
                    .loaded(p[0], p[2])
                    .then(|| held.contains(&p) || engine::is_anvil(&self.easyanvils_block(p)))
            });
            if !gone.is_empty() {
                ls.touch();
            }
        });
        for (p, slots) in gone {
            let at = BlockPos::new(p[0], p[1], p[2]);
            for stack in slots.into_iter().flatten() {
                for piece in horizon_world::level::removal::drop_item_stack(
                    &mut self.world.random,
                    at,
                    stack.count,
                ) {
                    self.spawn_item(
                        ItemStack { count: piece.count, ..stack.clone() },
                        piece.pos,
                        piece.motion,
                        10,
                    );
                }
            }
        }
    }

    /// The levels the take just paid (`menu.level_change`, 0 for a free rename).
    pub(super) fn easyanvils_took(level_change: i32) {
        FREE_TAKE.with(|f| f.set(level_change == 0));
    }

    /// `AnvilMenu.onTake`'s wear of the block, with Easy Anvils' chance (`onAnvilRepair`): 5 %,
    /// none for a rename alone. The vanilla 12 % is skipped by calling it as a creative take.
    pub(super) fn easyanvils_used(
        &mut self,
        pos: [i32; 3],
        creative: bool,
        original: impl FnOnce(&mut Self, [i32; 3], bool) -> bool,
    ) -> bool {
        let name = self.easyanvils_block(pos);
        let roll = if !creative && engine::is_anvil(&name) {
            self.world.random.next_float()
        } else {
            1.0
        };
        let right_empty = FREE_TAKE.with(std::cell::Cell::get);
        match engine::wear(&CFG, &name, right_empty, creative, roll) {
            Wear::None => original(self, pos, true),
            Wear::Becomes(to) => {
                let _ = self.set_state_named(pos, to, &["facing"]);
                self.level_event_at(1030, pos, 0);
                false
            }
            Wear::Break => {
                self.world.set(pos[0], pos[1], pos[2], self.world.air);
                self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
                self.level_event_at(1029, pos, 0);
                true
            }
        }
    }

    /// `ItemInteractionHandler.onUseBlock`: an iron block used on an anvil repairs one stage.
    pub(super) fn easyanvils_use(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        if !CFG.anvil_repairing || self.remote.is_some() {
            return false;
        }
        if !held.is_some_and(|s| &*s.item == "minecraft:iron_block") {
            return false;
        }
        let name = self.easyanvils_block(pos);
        let Some(to) = engine::repaired(&name) else {
            return false;
        };
        if self.set_state_named(pos, to, &["facing"]).is_err() {
            return false;
        }
        if !self.creative {
            let slot = self.held_mut();
            if let Some(stack) = slot {
                stack.count = stack.count.saturating_sub(1);
                if stack.count == 0 {
                    *slot = None;
                }
            }
        }
        // S2CAnvilRepairMessage: the anvil sound
        self.level_event_at(1030, pos, 0);
        true
    }

    /// The same by a dispenser (the registered behaviour of `Items.IRON_BLOCK`): `Some` when it was
    /// ours (an anvil in front), the slot's new content and the level event.
    pub(super) fn easyanvils_dispense(
        &mut self,
        pos: [i32; 3],
        step: [i32; 3],
        stack: &ItemStack,
    ) -> Option<(Option<ItemStack>, i32)> {
        if !CFG.anvil_repairing || &*stack.item != "minecraft:iron_block" {
            return None;
        }
        let target = [pos[0] + step[0], pos[1] + step[1], pos[2] + step[2]];
        let name = self.easyanvils_block(target);
        if !engine::is_anvil(&name) {
            return None;
        }
        match engine::repaired(&name) {
            Some(to) if self.set_state_named(target, to, &["facing"]).is_ok() => {
                self.level_event_at(1030, target, 0);
                let mut left = stack.clone();
                left.count -= 1;
                Some(((left.count > 0).then_some(left), 1000))
            }
            _ => Some((Some(stack.clone()), 1001)),
        }
    }

    /// `AnvilRenderer.render` for every anvil within 64 blocks.
    pub(super) fn easyanvils_geometry(
        &self,
        camera: glam::DVec3,
        out: &mut Vec<horizon_client::mesher::Vertex>,
    ) {
        if !engine::RENDER_ANVIL_CONTENTS {
            return;
        }
        let dim = self.dim.id();
        with_rt(|rt| {
            let store = if self.remote.is_some() {
                match &rt.mirror {
                    Some(m) if m.dim == dim => &m.store,
                    _ => return,
                }
            } else {
                match rt.levels.iter().find(|l| l.dim == dim && l.root == self.world_root) {
                    Some(ls) => &ls.store,
                    None => return,
                }
            };
            for (p, slots) in &store.anvils {
                if slots.iter().all(Option::is_none) {
                    continue;
                }
                let corner = glam::DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                if corner.distance_squared(camera) >= 64.0 * 64.0 {
                    continue;
                }
                let state = self.world.block(p[0], p[1], p[2]);
                let Some(info) = self.pack.assets.registry.state(state) else {
                    continue;
                };
                if !engine::is_anvil(&info.name) {
                    continue;
                }
                let facing = info.properties.get("facing").map_or("north", String::as_str);
                let light = horizon_client::block_entity_render::vertex_light(
                    self.world.light(p[0], p[1], p[2]),
                );
                let packed = BlockPos::new(p[0], p[1], p[2]).pack();
                for (i, stack) in slots.iter().enumerate() {
                    let Some(stack) = stack else {
                        continue;
                    };
                    let Some(pose) = engine::flat_pose(facing, i) else {
                        continue;
                    };
                    let seed = (packed as i32).wrapping_add(i as i32);
                    let item = self.resolve_item(stack, DisplayContext::Fixed, None, seed);
                    if item.is_empty() {
                        continue;
                    }
                    let m = glam::Mat4::from_translation(
                        (corner - camera).as_vec3() + glam::Vec3::from_array(pose.lift),
                    ) * glam::Mat4::from_rotation_x((-90f32).to_radians())
                        * glam::Mat4::from_rotation_z(pose.spin_z.to_radians())
                        * glam::Mat4::from_translation(glam::Vec3::from_array(pose.shift))
                        * glam::Mat4::from_scale(glam::Vec3::splat(pose.scale));
                    self.submit_item_world(&item, m, light, out);
                }
            }
        });
    }

    /// The world folder's files, written when the game saves.
    pub(super) fn easyanvils_save(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        let root = self.world_root.clone();
        let files: Vec<(PathBuf, String)> = with_rt(|rt| {
            rt.levels
                .iter_mut()
                .filter(|l| l.root == root && l.dirty)
                .map(|l| {
                    l.dirty = false;
                    (file_of(&l.root, l.dim), engine::save(&l.store, encode))
                })
                .collect()
        })
        .unwrap_or_default();
        for (path, text) in files {
            let tmp = path.with_extension("txt.tmp");
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&tmp, text))
                .and_then(|()| std::fs::rename(&tmp, &path));
            if let Err(e) = written {
                log::warn!("easyanvils: {} not saved: {e}", path.display());
            }
        }
    }

    /// A world opens: its anvils come from its folder again.
    pub(super) fn easyanvils_open() {
        with_rt(|rt| {
            rt.levels.clear();
            rt.bound = None;
            rt.guests.clear();
            rt.sent.clear();
        });
    }

    // ---- a server's guests (lan_stations.rs calls these with the guest's menu in hand) ----

    /// The block at `p` is an anvil (any wear).
    pub(super) fn easyanvils_is_anvil_at(&self, p: Pos) -> bool {
        engine::is_anvil(&self.easyanvils_block(p))
    }
    /// The anvil a guest holds: (position, window).
    pub(super) fn easyanvils_guest_bound(&self, conn: u32) -> Option<(Pos, i32)> {
        with_rt(|rt| rt.guests.get(&conn).map(|t| (t.pos, t.window))).flatten()
    }
    /// A guest's anvil menu at `pos` (window `window`) opened in the level in place: its two slots are the
    /// anvil's (the items to put in them), unless someone else holds it or his slots are not empty.
    pub(super) fn easyanvils_bind_guest(&mut self, conn: u32, pos: Pos, window: i32, empty: bool) -> Option<Vec<Option<ItemStack>>> {
        if self.world_root.as_os_str().is_empty() || !engine::is_anvil(&self.easyanvils_block(pos)) {
            return None;
        }
        with_rt(|rt| {
            let (root, dim) = (self.world_root.clone(), self.dim.id());
            if !engine::may_bind(&rt.holders(&root, dim), pos, &Holder::Guest(conn), empty) {
                return None;
            }
            rt.guests.insert(conn, GuestAnvil { root, dim, pos, window });
            Some(rt.level(self).store.open(pos))
        })
        .flatten()
    }
    /// The slots of a guest's held anvil, copied into the anvil.
    pub(super) fn easyanvils_mirror_guest(&mut self, conn: u32, inputs: &[Option<ItemStack>]) {
        with_rt(|rt| {
            let Some(t) = rt.guests.get(&conn) else { return };
            let (root, dim, pos) = (t.root.clone(), t.dim, t.pos);
            let ls = rt.level_at(&root, dim);
            if ls.store.mirror(pos, inputs) {
                ls.touch();
            }
        });
    }
    /// A guest's held anvil closes: its items stay in the anvil (the caller empties the guest's menu).
    pub(super) fn easyanvils_stash_guest(&mut self, conn: u32, inputs: &[Option<ItemStack>]) {
        with_rt(|rt| {
            let Some(t) = rt.guests.remove(&conn) else { return };
            let ls = rt.level_at(&t.root, t.dim);
            ls.store.mirror(t.pos, inputs);
            ls.touch();
        });
    }
    pub(super) fn easyanvils_unbind_guest(&mut self, conn: u32) {
        with_rt(|rt| rt.guests.remove(&conn));
    }
    /// A guest's held anvil closed by a path that gave its items back to him: the anvil's copy goes.
    pub(super) fn easyanvils_forget_guest(&mut self, conn: u32) {
        with_rt(|rt| {
            let Some(t) = rt.guests.remove(&conn) else { return };
            let ls = rt.level_at(&t.root, t.dim);
            if ls.store.mirror(t.pos, &[None, None]) {
                ls.touch();
            }
        });
    }
    /// The guests holding an anvil of the level in place: (connection, position, window).
    pub(super) fn easyanvils_guests_here(&self) -> Vec<(u32, Pos, i32)> {
        let dim = self.dim.id();
        with_rt(|rt| {
            rt.guests
                .iter()
                .filter(|(_, t)| t.root == self.world_root && t.dim == dim)
                .map(|(c, t)| (*c, t.pos, t.window))
                .collect()
        })
        .unwrap_or_default()
    }

    /// HEAD on `tick_guests` (after the held anvils): each guest with foundry in the level in place gets its
    /// anvils again when they changed (or when he is new here).
    pub(super) fn easyanvils_send_anvils(&mut self, host: &LanHost) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() || !engine::RENDER_ANVIL_CONTENTS {
            return;
        }
        let dim = self.dim.id();
        let out = with_rt(|rt| {
            rt.sent.retain(|c, _| host.hub.guest(*c).is_some());
            let generation = rt.level(self).generation;
            let want: Vec<u32> = host
                .hub
                .guests
                .iter()
                .filter(|g| host.hub.here(g) && g.loaded && super::foundry::guest_has_foundry(g.conn))
                .filter(|g| rt.sent.get(&g.conn) != Some(&(dim, generation)))
                .map(|g| g.conn)
                .collect();
            if want.is_empty() {
                return None;
            }
            for c in &want {
                rt.sent.insert(*c, (dim, generation));
            }
            Some((want, engine::save(&rt.level(self).store, encode)))
        })
        .flatten();
        if let Some((want, text)) = out {
            let body = ANVILS.encode(&(dim.to_owned(), text));
            for conn in want {
                super::foundry::send_to_guest(host, conn, ANVILS.id, &body);
            }
        }
    }

    /// HEAD on `native_tick` (every `Game`, the window included): a server's client takes the anvils its
    /// server sent.
    pub(super) fn easyanvils_client_tick(&mut self) {
        if self.remote.is_none() {
            return;
        }
        let got = super::foundry::client_take(ANVILS.id).into_iter().filter_map(|b| ANVILS.decode(&b)).last();
        let foundry = super::foundry::server_has_foundry();
        with_rt(|rt| {
            if let Some((dim, text)) = got {
                rt.mirror = Some(Mirror { dim, store: engine::load(&text, decode) });
            } else if !foundry {
                rt.mirror = None;
            }
        });
    }
}
