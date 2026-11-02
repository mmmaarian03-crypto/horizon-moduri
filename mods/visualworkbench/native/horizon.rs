// Visual Workbench inside Horizon's game (nightly 142388e1c136). The rules are the engine below
// (CraftingTableBlockEntity, WorkbenchBlockEntityRenderer of Visual Workbench 8.0.1). Horizon's
// crafting table has no block entity and `BlockEntity` cannot grow, so the tables' contents are a
// table of the mod (a file per dimension in the world folder, like the Torchmaster port): the 3x3
// grid of an open crafting table is that table's inventory, kept when the menu closes instead of
// going back to the player; the items hover above the block, turn to the nearest player and the
// last result floats over them.
//
// Where it runs (since Horizon #1514 single player's window is a client of its integrated server):
// - the tables' contents live where the world is: the in-frame host's game, or a server. A server's
//   guests (single player's window included) craft at a table through the server's menu engine
//   (`lan_crafting`): `visualworkbench` in `game/lan_crafting.rs` (native/lan_crafting.rs) fills the
//   guest's menu from the table when it opens, copies it back on every change and keeps the grid in
//   the table when it closes;
// - one player at a time holds a table's grid (`engine::may_bind`); another one gets a plain table;
// - the hovering items are the client's: a server sends each guest the tables of his level on forja's
//   channel (`visualworkbench:tables`, the world file's text, again whenever they change) and the client
//   turns and draws them.
use super::{Game, lan::LanHost};
#[allow(dead_code)]
mod engine {
    include!("visualworkbench_engine.rs");
}
use engine::{Anim, Pos, Store, Table};
use horizon_assets::item_model::DisplayContext;
use horizon_client::items::ItemStack;
use horizon_core::position::BlockPos;
use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
};

const TABLE: &str = "minecraft:crafting_table";
/// Server -> client: (dimension, the tables of that level as the world file's text).
const TABLES: super::forja::Channel<(String, String)> = super::forja::Channel::new("visualworkbench:tables");

struct LevelState {
    root: PathBuf,
    dim: &'static str,
    store: Store<ItemStack>,
    anims: HashMap<Pos, Anim>,
    dirty: bool,
    /// Bumped on every change of `store`: the guests' clients get the tables again.
    generation: u64,
}
impl LevelState {
    fn touch(&mut self) {
        self.dirty = true;
        self.generation += 1;
    }
}

/// A server guest's open table whose grid he holds.
struct GuestTable {
    root: PathBuf,
    dim: &'static str,
    pos: Pos,
    /// The guest's window of that menu.
    window: i32,
}

/// Who holds a table's grid.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Holder {
    Host,
    Guest(u32),
}

/// A server's client: the tables of its level, as its server sent them.
struct Mirror {
    dim: String,
    store: Store<ItemStack>,
    anims: HashMap<Pos, Anim>,
}

#[derive(Default)]
struct Runtime {
    levels: Vec<LevelState>,
    /// The table whose menu the in-frame host has open and whose grid is mirrored.
    bound: Option<(PathBuf, &'static str, Pos)>,
    /// A server's guests at a table, by connection.
    guests: HashMap<u32, GuestTable>,
    /// What each guest's client last got: (dimension, generation).
    sent: HashMap<u32, (&'static str, u64)>,
    /// A server's client.
    mirror: Option<Mirror>,
}

thread_local! {
    static RT: RefCell<Runtime> = RefCell::new(Runtime::default());
}
/// The runtime of this thread; a nested call (which no hook makes) gets `None`, not a panic.
fn with_rt<T>(f: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    RT.with(|rt| rt.try_borrow_mut().ok().map(|mut rt| f(&mut rt)))
}

fn file_of(root: &Path, dim: &str) -> PathBuf {
    root.join("data")
        .join("visualworkbench")
        .join(format!("{}.txt", dim.replace([':', '/'], "_")))
}

fn encode(stack: &ItemStack) -> String {
    horizon_client::item_data::canon_stack(Some(stack))
}
fn decode(text: &str) -> Option<ItemStack> {
    horizon_client::item_data::parse_stack(text).ok().flatten()
}

/// `getNearestPlayer(x + .5, y + .5, z + .5, 3.0, false)`: the player's offset when within 3 blocks.
fn near(player: Option<horizon_core::geometry::Vec3>, p: Pos) -> Option<(f64, f64)> {
    player.and_then(|q| {
        let d0 = q.x - (f64::from(p[0]) + 0.5);
        let d1 = q.z - (f64::from(p[2]) + 0.5);
        let dy = q.y - (f64::from(p[1]) + 0.5);
        (d0 * d0 + dy * dy + d1 * d1 < 9.0).then_some((d0, d1))
    })
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
                    anims: HashMap::new(),
                    dirty: false,
                    generation: 1,
                });
                self.levels.len() - 1
            }
        };
        &mut self.levels[index]
    }
    /// Who holds which table of a level.
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
    pub(super) fn visualworkbench_is_table(&self, p: Pos) -> bool {
        let state = self.world.block(p[0], p[1], p[2]);
        &*self.res.facts().get(state).name == TABLE
    }

    /// The open crafting table of the in-frame host's own menu. A server's guest crafts in the server's
    /// menu engine (`lan_crafting`), not in the game's own grid: inside his turn `craft_table` is not his
    /// table's menu (game/lan_crafting.rs handles it); a server's client has no tables of its own.
    fn visualworkbench_open_table(&self) -> Option<Pos> {
        if self.remote.is_some() || self.lan_acting.is_some() || self.craft_size != 3 {
            return None;
        }
        self.craft_table
    }

    /// `ModCraftingMenu` over the block entity: the grid of the open table is the table's
    /// inventory. First tick of an open table: its items fill the grid (if nobody else holds the
    /// table and the grid is empty); after that the grid is copied back (and the result kept) every
    /// tick, so a crash loses nothing.
    fn visualworkbench_sync_open(&mut self) {
        let Some(pos) = self.visualworkbench_open_table() else {
            with_rt(|rt| rt.bound = None);
            return;
        };
        if !self.visualworkbench_is_table(pos) {
            // The block is gone (`tick_craft_table` closes the screen next): its items stay in the
            // table's contents, which the cleanup spills; none go back with the player.
            self.visualworkbench_stash(pos);
            return;
        }
        let result = self.craft_recipe().map(|r| r.1);
        let grid: Vec<Option<ItemStack>> = self.craft_grid[..9].to_vec();
        let empty = grid.iter().all(Option::is_none);
        let loaded = with_rt(|rt| {
            let (root, dim) = (self.world_root.clone(), self.dim.id());
            let first = rt.bound.as_ref() != Some(&(root.clone(), dim, pos));
            if first {
                if !engine::may_bind(&rt.holders(&root, dim), pos, &Holder::Host, empty) {
                    // A plain table for the host: his own items go back to him.
                    rt.bound = None;
                    return None;
                }
                rt.bound = Some((root, dim, pos));
                Some(engine::open_table(&mut rt.level(self).store, pos))
            } else {
                let ls = rt.level(self);
                if engine::mirror(&mut ls.store, pos, &grid, result) {
                    ls.touch();
                }
                None
            }
        })
        .flatten();
        if let Some(items) = loaded {
            for (i, item) in items.into_iter().enumerate() {
                self.craft_grid[i] = item;
            }
        }
    }

    /// The grid goes into the table's contents and out of the player's hands.
    fn visualworkbench_stash(&mut self, pos: Pos) {
        let bound = with_rt(|rt| rt.bound.take()).flatten();
        if bound.is_none() {
            return;
        }
        let result = self.craft_recipe().map(|r| r.1);
        let items: Vec<Option<ItemStack>> = (0..9).map(|i| self.craft_grid[i].take()).collect();
        with_rt(|rt| {
            let ls = rt.level(self);
            engine::mirror(&mut ls.store, pos, &items, result);
            ls.touch();
        });
    }

    /// `Game::close_container` first: the 3x3 grid of a table stays in the table.
    pub(super) fn visualworkbench_close(&mut self) {
        if let Some(pos) = self.visualworkbench_open_table()
            && with_rt(|rt| rt.bound.is_some()).unwrap_or(false)
        {
            self.visualworkbench_stash(pos);
        }
    }

    /// Every game tick of a level (the in-frame host's, a server's): the in-frame host's open table, the
    /// turning of the items near the in-frame player, and the contents of tables whose block is gone
    /// (`CraftingTableBlock.onRemove`: `dropContents`; a table a guest holds is judged with his menu, in
    /// `tick_guests`).
    pub(super) fn visualworkbench_tick(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        self.visualworkbench_sync_open();
        let player = self.player.as_ref().map(|p| p.position);
        let mut gone: Vec<(Pos, Table<ItemStack>)> = Vec::new();
        with_rt(|rt| {
            let (root, dim) = (self.world_root.clone(), self.dim.id());
            let held: Vec<Pos> = rt
                .holders(&root, dim)
                .into_iter()
                .filter(|(_, h)| matches!(h, Holder::Guest(_)))
                .map(|(p, _)| p)
                .collect();
            let ls = rt.level(self);
            gone = engine::take_gone(&mut ls.store, |p| {
                (self.world.loaded(p[0], p[2])).then(|| held.contains(&p) || self.visualworkbench_is_table(p))
            });
            if !gone.is_empty() {
                ls.touch();
            }
            for (p, _) in &gone {
                ls.anims.remove(p);
            }
            for (p, t) in &ls.store.tables {
                ls.anims.entry(*p).or_default().tick(t.is_empty(), near(player, *p));
            }
        });
        for (p, t) in gone {
            self.visualworkbench_spill(p, t);
        }
    }

    /// `Containers.dropContents`: each stack at its own spot of the block.
    fn visualworkbench_spill(&mut self, p: Pos, table: Table<ItemStack>) {
        let at = BlockPos::new(p[0], p[1], p[2]);
        for stack in table.slots.into_iter().flatten() {
            for piece in
                horizon_world::level::removal::drop_item_stack(&mut self.world.random, at, stack.count)
            {
                self.spawn_item(
                    ItemStack { count: piece.count, ..stack.clone() },
                    piece.pos,
                    piece.motion,
                    10,
                );
            }
        }
    }

    // ---- a server's guests (lan_crafting.rs calls these with the guest's menu in hand) ----

    /// The table a guest holds: (position, window).
    pub(super) fn visualworkbench_guest_bound(&self, conn: u32) -> Option<(Pos, i32)> {
        with_rt(|rt| rt.guests.get(&conn).map(|t| (t.pos, t.window))).flatten()
    }
    /// A guest's menu of the table at `pos` (window `window`) opened in the level in place: his grid is the
    /// table's (the items to put in it), unless someone else holds it or his grid is not empty.
    pub(super) fn visualworkbench_bind_guest(&mut self, conn: u32, pos: Pos, window: i32, grid_empty: bool) -> Option<Vec<Option<ItemStack>>> {
        if self.world_root.as_os_str().is_empty() || !self.visualworkbench_is_table(pos) {
            return None;
        }
        with_rt(|rt| {
            let (root, dim) = (self.world_root.clone(), self.dim.id());
            if !engine::may_bind(&rt.holders(&root, dim), pos, &Holder::Guest(conn), grid_empty) {
                return None;
            }
            rt.guests.insert(conn, GuestTable { root, dim, pos, window });
            Some(engine::open_table(&mut rt.level(self).store, pos))
        })
        .flatten()
    }
    /// The grid and result of a guest's held table, copied into the table.
    pub(super) fn visualworkbench_mirror_guest(&mut self, conn: u32, grid: &[Option<ItemStack>], result: Option<ItemStack>) {
        with_rt(|rt| {
            let Some(t) = rt.guests.get(&conn) else { return };
            let (root, dim, pos) = (t.root.clone(), t.dim, t.pos);
            let ls = rt.level_at(&root, dim);
            if engine::mirror(&mut ls.store, pos, grid, result) {
                ls.touch();
            }
        });
    }
    /// A guest's held table closes: its grid stays in the table (the caller empties the guest's menu).
    pub(super) fn visualworkbench_stash_guest(&mut self, conn: u32, grid: &[Option<ItemStack>], result: Option<ItemStack>) {
        with_rt(|rt| {
            let Some(t) = rt.guests.remove(&conn) else { return };
            let ls = rt.level_at(&t.root, t.dim);
            engine::mirror(&mut ls.store, t.pos, grid, result);
            ls.touch();
        });
    }
    pub(super) fn visualworkbench_unbind_guest(&mut self, conn: u32) {
        with_rt(|rt| rt.guests.remove(&conn));
    }
    /// A guest's held table closed by a path that gave the grid back to him: the table's copy goes (its last
    /// result stays on show), so nothing is doubled.
    pub(super) fn visualworkbench_forget_guest(&mut self, conn: u32) {
        with_rt(|rt| {
            let Some(t) = rt.guests.remove(&conn) else { return };
            let ls = rt.level_at(&t.root, t.dim);
            let last = ls.store.tables.get(&t.pos).and_then(|x| x.last.clone());
            if engine::mirror(&mut ls.store, t.pos, &vec![None; engine::SLOTS], last) {
                ls.touch();
            }
        });
    }
    /// The guests holding a table of the level in place: (connection, position, window).
    pub(super) fn visualworkbench_guests_here(&self) -> Vec<(u32, Pos, i32)> {
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

    /// HEAD on `tick_guests` (part 2, after the held tables): each guest with forja in the level in place
    /// gets its tables again when they changed (or when he is new here).
    pub(super) fn visualworkbench_send_tables(&mut self, host: &LanHost) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
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
                .filter(|g| host.hub.here(g) && g.loaded && super::forja::guest_has_forja(g.conn))
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
            let body = TABLES.encode(&(dim.to_owned(), text));
            for conn in want {
                super::forja::send_to_guest(host, conn, TABLES.id, &body);
            }
        }
    }

    /// HEAD on `native_tick` (every `Game`, the window included): a server's client takes the tables its
    /// server sent and turns their items to its player (the animation is the client's, as in the original).
    pub(super) fn visualworkbench_client_tick(&mut self) {
        if self.remote.is_none() {
            return;
        }
        let got = super::forja::client_take(TABLES.id).into_iter().filter_map(|b| TABLES.decode(&b)).last();
        let forja = super::forja::server_has_forja();
        let player = self.player.as_ref().map(|p| p.position);
        with_rt(|rt| {
            if let Some((dim, text)) = got {
                let store = engine::load(&text, decode);
                let mut anims = rt.mirror.take().filter(|m| m.dim == dim).map(|m| m.anims).unwrap_or_default();
                anims.retain(|p, _| store.tables.contains_key(p));
                rt.mirror = Some(Mirror { dim, store, anims });
            } else if !forja {
                rt.mirror = None;
            }
            if let Some(m) = rt.mirror.as_mut() {
                for (p, t) in &m.store.tables {
                    m.anims.entry(*p).or_default().tick(t.is_empty(), near(player, *p));
                }
            }
        });
    }

    /// `WorkbenchBlockEntityRenderer.render` for every table within 64 blocks (a server's client: the
    /// tables its server sent).
    pub(super) fn visualworkbench_geometry(
        &self,
        camera: glam::DVec3,
        out: &mut Vec<horizon_client::mesher::Vertex>,
    ) {
        let partial = self.clock.fraction() as f32;
        let dim = self.dim.id();
        with_rt(|rt| {
            let (store, anims) = if self.remote.is_some() {
                match &rt.mirror {
                    Some(m) if m.dim == dim => (&m.store, &m.anims),
                    _ => return,
                }
            } else {
                match rt.levels.iter().find(|l| l.dim == dim && l.root == self.world_root) {
                    Some(ls) => (&ls.store, &ls.anims),
                    None => return,
                }
            };
            for (p, t) in &store.tables {
                if t.is_blank() {
                    continue;
                }
                let corner = glam::DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
                if corner.distance_squared(camera) >= 64.0 * 64.0 {
                    continue;
                }
                let anim = anims.get(p).cloned().unwrap_or_default();
                // The block itself is solid: the light is the block above's.
                let light = horizon_client::block_entity_render::vertex_light(
                    self.world.light(p[0], p[1] + 1, p[2]),
                );
                let rel = (corner - camera).as_vec3();
                let packed = BlockPos::new(p[0], p[1], p[2]).pack();
                for (i, stack) in t.slots.iter().enumerate() {
                    let Some(stack) = stack else {
                        continue;
                    };
                    let seed = (packed as i32).wrapping_add(i as i32);
                    let item = self.resolve_item(stack, DisplayContext::Fixed, None, seed);
                    if item.is_empty() {
                        continue;
                    }
                    let pose = engine::ingredient_pose(&anim, partial, i, item.uses_block_light());
                    let mut m = glam::Mat4::from_translation(rel + glam::Vec3::from_array(pose.first))
                        * glam::Mat4::from_rotation_y(pose.yaw.to_radians())
                        * glam::Mat4::from_translation(glam::Vec3::from_array(pose.second));
                    if pose.flat {
                        m = m
                            * glam::Mat4::from_rotation_x(90f32.to_radians())
                            * glam::Mat4::from_rotation_z(180f32.to_radians());
                    }
                    m *= glam::Mat4::from_scale(glam::Vec3::splat(pose.scale));
                    self.submit_item_world(&item, m, light, out);
                }
                if engine::RENDER_RESULT
                    && let Some(last) = &t.last
                {
                    let item = self.resolve_item(last, DisplayContext::Ground, None, 0);
                    if item.is_empty() {
                        continue;
                    }
                    let block_item = item.uses_block_light();
                    // The model's GROUND scale: blocks 0.25, flat items 0.5 (the vanilla defaults).
                    let rp = engine::result_pose(
                        anim.ticks as f32 + partial,
                        block_item,
                        if block_item { 0.25 } else { 0.5 },
                    );
                    let m = glam::Mat4::from_translation(rel + glam::Vec3::from_array(rp.at))
                        * glam::Mat4::from_rotation_y(rp.yaw)
                        * glam::Mat4::from_scale(glam::Vec3::splat(rp.scale));
                    self.submit_item_world(&item, m, light, out);
                }
            }
        });
    }

    /// The world folder's files, written when the game saves.
    pub(super) fn visualworkbench_save(&mut self) {
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
                log::warn!("visualworkbench: {} not saved: {e}", path.display());
            }
        }
    }

    /// A world opens: its tables come from its folder again.
    pub(super) fn visualworkbench_open() {
        with_rt(|rt| {
            rt.levels.clear();
            rt.bound = None;
            rt.guests.clear();
            rt.sent.clear();
        });
    }
}
