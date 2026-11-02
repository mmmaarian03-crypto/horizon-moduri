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
//   reads the choice of the guest whose click it runs. The choice travels on foundry's channel
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
    include!("polymorph_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "polymorph";
/// The remembered recipe id, read by `crate::menu::polymorph` in the lib: `<temp dir>/<this>-<pid>-<thread>.txt`.
const PICK_FILE: &str = "horizon-polymorph-pick";
/// The key of a guest's choice in his player data (`Guest::saved`).
const KEY: &str = "PolymorphSelection";
/// Client -> server: the recipe the player chose in the popup (`ServerboundSelectRecipe` of the original).
const PICK: super::foundry::Channel<String> = super::foundry::Channel::new("polymorph:pick");
/// Server -> client: the guest's remembered choice, once when he joins.
const SEL: super::foundry::Channel<Option<String>> = super::foundry::Channel::new("polymorph:sel");

/// The server half of `polymorph:pick` (idempotent; the registry is the process's).
fn polymorph_register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::foundry::register_server_channel(PICK.id, polymorph_pick_from_client);
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

include!("kit_state.rs");

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
    /// (single player's window included) needs foundry on its server, which crafts the choice.
    fn polymorph_kind(&self) -> Option<engine::Kind> {
        if self.screen != Screen::Inventory || self.container.is_some() {
            return None;
        }
        if self.remote.is_some() && !super::foundry::server_has_foundry() {
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
    /// A server's client shows the result its server will craft (the choice it sent); without foundry there
    /// the server crafts the first match.
    pub(super) fn polymorph_craft(&self) -> Option<(String, ItemStack)> {
        if self.remote.is_some() && !super::foundry::server_has_foundry() {
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
                    super::foundry::send_to_server(self, PICK.id, &PICK.encode(&id));
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

    /// HEAD on `tick_guests` (a server): each guest with foundry is told his remembered choice once.
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
                .filter(|g| g.loaded && !sent.contains(&g.conn) && super::foundry::guest_has_foundry(g.conn))
                .map(|g| (g.conn, read_saved(&g.saved)))
                .collect();
            sent.extend(new.iter().map(|(c, _)| *c));
            new
        });
        for (conn, choice) in new {
            super::foundry::send_to_guest(host, conn, SEL.id, &SEL.encode(&choice));
        }
    }

    /// HEAD on `native_tick` (every `Game`, the window included): a server's client takes the choice its
    /// server remembers for it; without foundry there it has none.
    pub(super) fn polymorph_client_tick(&mut self) {
        if self.remote.is_none() {
            return;
        }
        let got = super::foundry::client_take(SEL.id).into_iter().filter_map(|b| SEL.decode(&b)).last();
        let foundry = super::foundry::server_has_foundry();
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(choice) = got {
                s.remembered = choice;
                s.loaded = true;
                s.open = false;
                publish(&s.remembered);
            } else if !foundry && s.remembered.is_some() {
                s.remembered = None;
                publish(&None);
            }
        });
    }
}
