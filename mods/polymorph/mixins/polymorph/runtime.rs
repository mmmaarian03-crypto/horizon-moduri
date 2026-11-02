// Polymorph inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs).
// The game side: the crafting screens (inventory 2x2, crafting table) get Polymorph's selector button and
// popup; the chosen recipe is crafted instead of the first match. The menu engine (the lib crate's
// `Menu::recompute`, mixins/polymorph/menu.rs) makes the result from the same choice, handed over in the
// file below (the game is a bin over a lib that cannot see it, and the bin forbids `unsafe`, so no shared
// Rust item or environment variable can carry it).
//
// Where it runs (since Horizon #1514 single player's window is a client of its integrated server):
// - the selector and the popup are the client's (the window draws them over its crafting screen; the result
//   slot a server's client shows is computed from its own grid, `craft_recipe`);
// - the crafting itself is the server's: a guest's grids are the server's menu engine (`lan_crafting`), which
//   reads the choice of the guest whose click it runs. The choice travels on forja's channel
//   (`polymorph:pick` client -> server, `polymorph:sel` server -> client when the guest joins) and is kept in
//   the guest's own player data (`saved`); the in-frame host keeps the world's `selection.txt`;
// - the hand-over file is per process and thread (`pick_path`): the window and the integrated server are two
//   threads of one process, and two games on one PC are two processes. The server writes the acting guest's
//   choice into its thread's file before each of the guest's crafting clicks.
#[allow(unused_imports)]
use super::{Game, ItemStack, Result, Screen, lan::LanHost};
use horizon_client::{
    crafting::{Recipe, Recipes},
    hud::{HudBatch, rgba},
};
use horizon_world::nbt::{JavaString, Tag};
use std::cell::RefCell;

#[allow(dead_code)]
mod engine {
    // Polymorph for Horizon: the rules, without any game code.
// Written from Polymorph 0.49.10 for Minecraft 1.20.1 (TheIllusiveC4 / illusivesoulworks, LGPL-3.0-or-later;
// behaviour read from its public description and jar assets; no code of it is used) and the pack's config.
// This file is pasted into the game as `crate::game::polymorph::engine` (tools/hnmod.py bundle) and is also the
// `horizon_polymorph` crate the tests use: plain Rust, no game types, no inner attributes (`#![...]`).
//
// What Polymorph does: when the crafting grid matches several recipes (different results), vanilla
// always makes the first one. Polymorph lists every match, lets the player pick one and remembers the
// pick: the remembered recipe is used whenever it still matches, otherwise the first match.

/// the pack's `polymorph-integrations.toml` (the only Polymorph config he has). They switch on
/// compatibility with the FastFurnace / FastSuite / FastBench mods, which are 1.20.1 Forge speed-ups
/// with no Horizon counterpart: nothing to port.
#[allow(dead_code)]
pub mod config {
    pub const FASTFURNACE: bool = true; // polymorph-integrations.toml:3 [] fastfurnace
    pub const FASTSUITE: bool = true; // polymorph-integrations.toml:4 [] fastsuite
    pub const FASTBENCH: bool = true; // polymorph-integrations.toml:5 [] fastbench
}

/// At most this many alternatives are shown (the popup has 6 columns of 3).
pub const MAX_ALTS: usize = 18;
/// Output button size, from the original's 25x25 `output_button` sprite.
pub const BUTTON: f32 = 25.0;
/// Selector (toggle) button size, from the original's 16x16 `selector_button` sprite.
pub const TOGGLE: f32 = 16.0;
const ROWS: usize = 3;

/// One recipe that matches the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alt {
    /// Recipe id (file name without namespace), as the game's recipe list names it.
    pub id: String,
    pub item: String,
    pub count: u32,
}

/// The matches in the order Polymorph offers them: the game's own first match first (what vanilla
/// crafts), then the others in recipe order, no recipe twice, at most `MAX_ALTS`.
pub fn order(first: Option<Alt>, rest: Vec<Alt>) -> Vec<Alt> {
    let mut out: Vec<Alt> = Vec::new();
    for a in first.into_iter().chain(rest) {
        if out.len() < MAX_ALTS && !out.iter().any(|o| o.id == a.id) {
            out.push(a);
        }
    }
    out
}

/// Index of the recipe that is crafted: the remembered one while it still matches, else the first.
pub fn pick(alts: &[Alt], remembered: Option<&str>) -> usize {
    remembered
        .and_then(|r| alts.iter().position(|a| a.id == r))
        .unwrap_or(0)
}

/// Which crafting screen (its result slot decides where the buttons go).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The 2x2 grid of the inventory screen (result slot at 154,28).
    Inventory,
    /// The crafting table (result slot at 124,35).
    Table,
}
pub type Rect = (f32, f32, f32, f32);

/// The selector button, relative to the screen's top-left corner: just above the result slot.
pub fn toggle(kind: Kind) -> Rect {
    match kind {
        Kind::Table => (124.0, 15.0, TOGGLE, TOGGLE),
        Kind::Inventory => (154.0, 8.0, TOGGLE, TOGGLE),
    }
}
/// The popup's output buttons (`n` of them): columns of three, read top to bottom then left to
/// right, the last column touching the selector button's right edge.
pub fn buttons(kind: Kind, n: usize) -> Vec<Rect> {
    let n = n.min(MAX_ALTS);
    let t = toggle(kind);
    let cols = n.div_ceil(ROWS);
    let left = (t.0 + TOGGLE - cols as f32 * BUTTON).max(0.0);
    (0..n)
        .map(|i| {
            (
                left + (i / ROWS) as f32 * BUTTON,
                t.1 + TOGGLE + 2.0 + (i % ROWS) as f32 * BUTTON,
                BUTTON,
                BUTTON,
            )
        })
        .collect()
}
fn inside(r: Rect, x: f32, y: f32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}
/// The popup's whole area.
fn popup(kind: Kind, n: usize) -> Option<Rect> {
    let b = buttons(kind, n);
    let first = b.first()?;
    let right = b.iter().map(|r| r.0 + r.2).fold(f32::MIN, f32::max);
    let rows = n.min(ROWS) as f32;
    Some((first.0, first.1, right - first.0, rows * BUTTON))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// Not ours: the game handles the click.
    Pass,
    /// The selector button: open or close the popup.
    Toggle,
    /// An output button: craft alternative `i` from now on.
    Choose(usize),
    /// Inside the popup but on no button: eaten (it must not click the slot under it).
    Swallow,
}
/// A left click at (`x`,`y`) relative to the screen's corner, with `n` matching recipes.
/// Nothing is shown (and nothing eaten) while fewer than two recipes match.
pub fn click(kind: Kind, open: bool, n: usize, x: f32, y: f32) -> Click {
    if n < 2 {
        return Click::Pass;
    }
    if inside(toggle(kind), x, y) {
        return Click::Toggle;
    }
    if !open {
        return Click::Pass;
    }
    if let Some(i) = buttons(kind, n).iter().position(|r| inside(*r, x, y)) {
        return Click::Choose(i);
    }
    match popup(kind, n) {
        Some(p) if inside(p, x, y) => Click::Swallow,
        _ => Click::Pass,
    }
}

/// The remembered recipe, saved in `<world>/data/polymorph/<dimension>.txt` (kit_state): one line, the
/// recipe id (the original keeps it in the player's data).
pub fn load(text: Option<String>) -> Option<String> {
    text.and_then(|t| t.lines().next().map(|l| l.trim().to_owned()))
        .filter(|l| !l.is_empty())
}
pub fn save(remembered: Option<&str>) -> String {
    remembered.map(|r| format!("{r}\n")).unwrap_or_default()
}

}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "polymorph";
/// The remembered recipe id, read by `crate::menu::polymorph` in the lib: `<temp dir>/<this>-<pid>-<thread>.txt`.
const PICK_FILE: &str = "horizon-polymorph-pick";
/// The key of a guest's choice in his player data (`Guest::saved`).
const KEY: &str = "PolymorphSelection";
/// Client -> server: the recipe the player chose in the popup (`ServerboundSelectRecipe` of the original).
const PICK: super::forja::Channel<String> = super::forja::Channel::new("polymorph:pick");
/// Server -> client: the guest's remembered choice, once when he joins.
const SEL: super::forja::Channel<Option<String>> = super::forja::Channel::new("polymorph:sel");

/// The server half of `polymorph:pick` (idempotent; the registry is the process's).
fn polymorph_register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::forja::register_server_channel(PICK.id, polymorph_pick_from_client);
}
/// A guest chose a recipe: it is his from now on (his player data, saved with him).
fn polymorph_pick_from_client(_game: &mut Game<'_>, host: &mut LanHost, conn: u32, body: &[u8]) {
    let Some(id) = PICK.decode(body) else { return };
    if let Some(g) = host.hub.guests.iter_mut().find(|g| g.conn == conn) {
        write_saved(&mut g.saved, Some(&id));
    }
}
/// A guest's remembered recipe in his player data.
fn read_saved(saved: &Option<Tag>) -> Option<String> {
    let Some(Tag::Compound(map)) = saved else { return None };
    match map.get(&JavaString::from(KEY)) {
        Some(Tag::String(id)) => id.to_string().ok().filter(|id| !id.is_empty()),
        _ => None,
    }
}
fn write_saved(saved: &mut Option<Tag>, id: Option<&str>) {
    let e = saved.get_or_insert_with(|| Tag::Compound(Default::default()));
    let Tag::Compound(map) = e else { return };
    match id {
        Some(id) => {
            map.insert(JavaString::from(KEY), Tag::String(JavaString::from(id)));
        }
        None => {
            map.remove(&JavaString::from(KEY));
        }
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

type Matches = Vec<(engine::Alt, ItemStack)>;

#[derive(Default)]
struct Store {
    remembered: Option<String>,
    loaded: bool,
    dirty: bool,
    /// The popup is open.
    open: bool,
    /// The matches of the last grid: (size, item ids of the cells) -> the matches and their stacks.
    cache: Option<((usize, Vec<Option<String>>), Matches)>,
}
thread_local! {
    /// The window's player (a server's client, or the in-frame host).
    static STORE: RefCell<Store> = RefCell::new(Store::default());
    /// What this thread's file holds now (`None` = not written yet), so a click writes only a change.
    static PUBLISHED: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
    /// The guests a server already told their choice (`polymorph:sel`).
    static SENT: RefCell<std::collections::HashSet<u32>> = RefCell::new(Default::default());
}

/// This thread's hand-over file (the lib's `menu::polymorph::pick_path` makes the same name).
fn pick_path() -> std::path::PathBuf {
    let thread: String = format!("{:?}", std::thread::current().id()).chars().filter(char::is_ascii_digit).collect();
    std::env::temp_dir().join(format!("{PICK_FILE}-{}-{thread}.txt", std::process::id()))
}

/// Hands the choice to the lib's menu engine on this thread (no file = no choice).
fn publish(remembered: &Option<String>) {
    if PUBLISHED.with(|p| p.borrow().as_ref() == Some(remembered)) {
        return;
    }
    let path = pick_path();
    let done = match remembered {
        Some(id) => std::fs::write(&path, id),
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
    };
    match done {
        Ok(()) => PUBLISHED.with(|p| *p.borrow_mut() = Some(remembered.clone())),
        Err(e) => log::warn!("polymorph: {} not updated: {e}", path.display()),
    }
}

/// The hand-over files of games gone a day ago (one per process and thread).
fn sweep_old_files() {
    let Ok(dir) = std::fs::read_dir(std::env::temp_dir()) else { return };
    let day = std::time::Duration::from_secs(24 * 3600);
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with(PICK_FILE) {
            continue;
        }
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > day);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The stack a Shaped/Shapeless recipe makes, by id (`Special` ones have code of their own: none).
fn plain_result(recipes: &Recipes, id: &str) -> Option<ItemStack> {
    match &recipes.crafting.iter().find(|(n, _)| n == id)?.1 {
        Recipe::Shaped { result, .. } | Recipe::Shapeless { result, .. } => Some(result.create()),
        Recipe::Special(_) => None,
    }
}

/// Every recipe that matches the grid, the game's own first match first, with what each makes.
fn alternatives(recipes: &Recipes, grid: &[Option<ItemStack>], size: usize) -> Matches {
    if grid.iter().all(Option::is_none) {
        return Vec::new();
    }
    let alt = |id: &str, r: &ItemStack| engine::Alt {
        id: id.to_owned(),
        item: r.item.to_string(),
        count: r.count,
    };
    let first = recipes.find(grid, size).map(|(id, r)| (alt(id, &r), r));
    let rest: Matches = recipes
        .crafting
        .iter()
        .filter_map(|(name, recipe)| match recipe {
            Recipe::Shaped { result, .. } | Recipe::Shapeless { result, .. }
                if recipes.recipe_matches(name, grid, size) =>
            {
                let r = result.create();
                Some((alt(name, &r), r))
            }
            _ => None,
        })
        .collect();
    let ordered = engine::order(
        first.as_ref().map(|f| f.0.clone()),
        rest.iter().map(|r| r.0.clone()).collect(),
    );
    ordered
        .into_iter()
        .filter_map(|a| {
            let stack = first
                .iter()
                .chain(rest.iter())
                .find(|(x, _)| x.id == a.id)
                .map(|(_, s)| s.clone())?;
            Some((a, stack))
        })
        .collect()
}

/// The recipe Polymorph crafts instead of the game's first match: the remembered one, when it matches
/// the grid. `None` = leave the game's result.
fn chosen(recipes: &Recipes, grid: &[Option<ItemStack>], size: usize) -> Option<(String, ItemStack)> {
    let id = STORE.with(|s| s.borrow().remembered.clone())?;
    if !recipes.recipe_matches(&id, grid, size) {
        return None;
    }
    let stack = plain_result(recipes, &id)?;
    Some((id, stack))
}

impl Game<'_> {
    /// The in-frame host's choice from the world's side file, once per world. A server's client gets its
    /// own from the server (`polymorph:sel`).
    fn polymorph_load(&self) {
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            if s.loaded {
                return;
            }
            s.loaded = true;
            if self.remote.is_some() {
                return;
            }
            s.remembered = engine::load(kit_state::read(self, "selection.txt"));
            publish(&s.remembered);
        });
    }
    fn polymorph_grid(&self) -> Vec<Option<ItemStack>> {
        let n = self.craft_size;
        (0..n * n).map(|i| self.craft_grid[i].clone()).collect()
    }
    /// The crafting screen Polymorph decorates: the inventory's 2x2 or a crafting table. A server's client
    /// (single player's window included) needs forja on its server, which crafts the choice.
    fn polymorph_kind(&self) -> Option<engine::Kind> {
        if self.screen != Screen::Inventory || self.container.is_some() {
            return None;
        }
        if self.remote.is_some() && !super::forja::server_has_forja() {
            return None;
        }
        if !self.slots().contains(&200) {
            return None;
        }
        match self.craft_size {
            3 => Some(engine::Kind::Table),
            2 => Some(engine::Kind::Inventory),
            _ => None,
        }
    }
    /// The matches of the grid in the open crafting screen (cached until the grid changes).
    fn polymorph_alts(&self) -> Matches {
        let grid = self.polymorph_grid();
        let key = (
            self.craft_size,
            grid.iter()
                .map(|c| c.as_ref().map(|s| s.item.to_string()))
                .collect::<Vec<_>>(),
        );
        if let Some(hit) = STORE.with(|s| {
            s.borrow()
                .cache
                .as_ref()
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return hit;
        }
        let alts = alternatives(self.res.recipes(), &grid, self.craft_size);
        STORE.with(|s| s.borrow_mut().cache = Some((key, alts.clone())));
        alts
    }

    /// HEAD (cancellable) on `Game::craft_recipe`: the result slot, the shift-click loop and the display
    /// use the chosen recipe; `None` leaves the game's own first match.
    /// A server's client shows the result its server will craft (the choice it sent); without forja there
    /// the server crafts the first match.
    pub(super) fn polymorph_craft(&self) -> Option<(String, ItemStack)> {
        if self.remote.is_some() && !super::forja::server_has_forja() {
            return None;
        }
        self.polymorph_load();
        chosen(self.res.recipes(), &self.polymorph_grid(), self.craft_size)
    }

    /// HEAD `draw_carried`: the selector button and, open, the output buttons.
    pub(super) fn polymorph_draw(&mut self, batch: &mut HudBatch) {
        self.polymorph_load();
        let Some(kind) = self.polymorph_kind() else {
            return;
        };
        let alts = self.polymorph_alts();
        if alts.len() < 2 {
            STORE.with(|s| s.borrow_mut().open = false);
            return;
        }
        let (open, current) = STORE.with(|s| {
            let s = s.borrow();
            let ids: Vec<engine::Alt> = alts.iter().map(|a| a.0.clone()).collect();
            (s.open, engine::pick(&ids, s.remembered.as_deref()))
        });
        let (ox, oy) = self.inventory_origin(batch);
        let (cx, cy) = (self.cursor.0 - ox, self.cursor.1 - oy);
        let over = |r: engine::Rect| cx >= r.0 && cx < r.0 + r.2 && cy >= r.1 && cy < r.1 + r.3;
        let button = "gui/sprites/widget/button";
        let hot = "gui/sprites/widget/button_highlighted";
        let t = engine::toggle(kind);
        let sprite = if over(t) || open { hot } else { button };
        horizon_client::ui::nine_slice(
            batch,
            &self.gui.atlas,
            sprite,
            ox + t.0,
            oy + t.1,
            t.2,
            t.3,
            u32::MAX,
        );
        // The original's selector button shows the crafted result; so does this one.
        self.draw_item(batch, &alts[current].1, ox + t.0, oy + t.1);
        batch.fill(&self.gui.atlas, ox + t.0, oy + t.1, t.2, t.3, rgba(0, 0, 0, 90));
        let mut tip: Option<ItemStack> = over(t).then(|| alts[current].1.clone());
        if open {
            for (i, r) in engine::buttons(kind, alts.len()).into_iter().enumerate() {
                let sprite = if i == current || over(r) { hot } else { button };
                horizon_client::ui::nine_slice(
                    batch,
                    &self.gui.atlas,
                    sprite,
                    ox + r.0,
                    oy + r.1,
                    r.2,
                    r.3,
                    u32::MAX,
                );
                self.draw_item(batch, &alts[i].1, ox + r.0 + 4.5, oy + r.1 + 4.5);
                if over(r) {
                    tip = Some(alts[i].1.clone());
                }
            }
        }
        if let Some(stack) = tip {
            self.draw_stack_tooltip(batch, &stack, self.cursor.0, self.cursor.1);
        }
    }

    /// HEAD (cancellable) `sort_mouse_press`: a left press on Polymorph's buttons is ours, not a slot click.
    pub(super) fn polymorph_press(&mut self, button: winit::event::MouseButton) -> bool {
        if button != winit::event::MouseButton::Left {
            return false;
        }
        let Some(kind) = self.polymorph_kind() else {
            return false;
        };
        self.polymorph_load();
        let alts = self.polymorph_alts();
        let Some(size) = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return false;
        };
        let (ox, oy) = self.inventory_origin(&HudBatch::new(size));
        let open = STORE.with(|s| s.borrow().open);
        match engine::click(kind, open, alts.len(), self.cursor.0 - ox, self.cursor.1 - oy) {
            engine::Click::Pass => false,
            engine::Click::Swallow => true,
            engine::Click::Toggle => {
                STORE.with(|s| s.borrow_mut().open = !open);
                self.audio.play_button_click();
                true
            }
            engine::Click::Choose(i) => {
                let id = alts[i].0.id.clone();
                let remote = self.remote.is_some();
                STORE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.remembered = Some(id.clone());
                    // A server's client: the server keeps it with the player.
                    s.dirty = !remote;
                    s.open = false;
                    publish(&s.remembered);
                });
                if remote {
                    super::forja::send_to_server(self, PICK.id, &PICK.encode(&id));
                }
                self.audio.play_button_click();
                true
            }
        }
    }

    /// HEAD `save_game_metadata`: the remembered recipe goes into the world's side file.
    pub(super) fn polymorph_save(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let text = STORE.with(|s| {
            let mut s = s.borrow_mut();
            let out = s.dirty.then(|| engine::save(s.remembered.as_deref()));
            if out.is_some() {
                s.dirty = false;
            }
            out
        });
        if let Some(text) = text
            && !kit_state::write(self, "selection.txt", &text)
        {
            STORE.with(|s| s.borrow_mut().dirty = true);
        }
    }
    /// HEAD `load_world_state` (no bindings): a world opens; its choice is read again on first use.
    pub(super) fn polymorph_world_opened() {
        STORE.with(|s| *s.borrow_mut() = Store::default());
        SENT.with(|s| s.borrow_mut().clear());
        publish(&None);
        polymorph_register();
        sweep_old_files();
    }

    /// HEAD on `menu_ops` (the window's own clicks): this thread's file holds the window player's choice. The
    /// in-frame host's server writes his guests' choices into the same thread's file before their clicks.
    pub(super) fn polymorph_before_menu_ops(&self) {
        if self.lan_acting.is_some() {
            return;
        }
        let mine = STORE.with(|s| s.borrow().remembered.clone());
        publish(&mine);
    }

    /// HEAD on the guests' crafting on a server (`lan_inventory_click`, `lan_crafting_click`,
    /// `lan_place_recipe`; single player's window is such a guest): the menu engine reads this guest's
    /// choice. A crafting table's menu stays open between clicks, so its result is made again from the
    /// guest's choice before the click (`table`).
    pub(super) fn polymorph_guest_op(&mut self, host: &mut LanHost, conn: u32, table: bool) {
        polymorph_register();
        let choice = host.hub.guest(conn).and_then(|g| read_saved(&g.saved));
        publish(&choice);
        if table {
            self.polymorph_recompute_table(host, conn);
        }
    }

    /// HEAD on `tick_guests` (a server): each guest with forja is told his remembered choice once.
    pub(super) fn polymorph_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        polymorph_register();
        let new: Vec<(u32, Option<String>)> = SENT.with(|sent| {
            let mut sent = sent.borrow_mut();
            sent.retain(|c| host.hub.guest(*c).is_some());
            let new: Vec<(u32, Option<String>)> = host
                .hub
                .guests
                .iter()
                .filter(|g| g.loaded && !sent.contains(&g.conn) && super::forja::guest_has_forja(g.conn))
                .map(|g| (g.conn, read_saved(&g.saved)))
                .collect();
            sent.extend(new.iter().map(|(c, _)| *c));
            new
        });
        for (conn, choice) in new {
            super::forja::send_to_guest(host, conn, SEL.id, &SEL.encode(&choice));
        }
    }

    /// HEAD on `native_tick` (every `Game`, the window included): a server's client takes the choice its
    /// server remembers for it; without forja there it has none.
    pub(super) fn polymorph_client_tick(&mut self) {
        if self.remote.is_none() {
            return;
        }
        let got = super::forja::client_take(SEL.id).into_iter().filter_map(|b| SEL.decode(&b)).last();
        let forja = super::forja::server_has_forja();
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(choice) = got {
                s.remembered = choice;
                s.loaded = true;
                s.open = false;
                publish(&s.remembered);
            } else if !forja && s.remembered.is_some() {
                s.remembered = None;
                publish(&None);
            }
        });
    }
}
