// Patchouli inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the books
// (book.json, categories, entries, templates), the text markup and its layout, the pages, the reading state and the
// navigation. This part reads the game: the items, the recipes, the advancements, the font, the language, and draws
// what the engine lists.
//
// The screen: Horizon has no way to register a new screen, so the book takes over the Advancements screen (a plain
// screen over the world that pauses a single-player game, as Patchouli's `GuiBook` does when `pause_game` is set):
// hooks on its draw / scroll / drag, the mouse (`sort_mouse_press`: left, right, back and forward buttons), the keys
// (`sort_key_press`) and the typed text (`recipe_search_focused` + `recipe_key`, for the search bar of the lists).
// The book opens from the use of its item: `books::remote_use_book` on the window of a single-player or server game,
// `books::use_book` when the world runs in the frame.
#[allow(unused_imports)]
use super::{Game, Result, Screen};
use horizon_client::hud::{HudBatch, Rect, rgba};
use horizon_client::items::ItemStack;
use horizon_client::text::{Style as TStyle, Text};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// The rules. `pub(in crate::game)` so that the native mods of the books (Botania, Blood Magic...) can reach
/// `crate::game::patchouli::engine::{register_processor, Processor, Vars, Env}` when they are built together with this one.
#[allow(dead_code)]
pub(in crate::game) mod engine {
    include!("patchouli_engine.rs");
}

/// Used by `native_data_dir` / `native_setting`: `patchouli_data.json` lives there, as `patchouli_data.json` does in the
/// game folder of the original.
const ID: &str = "patchouli";
const DATA_KEY: &str = "patchouli_data.json";
/// `ItemModBook.TAG_BOOK`.
const TAG_BOOK: &str = "patchouli:book";
const UNIFORM: &str = "minecraft:uniform";

// ------------------------------------------------------------------------------------------------
// State
// ------------------------------------------------------------------------------------------------

type Listed = (engine::BookRef, std::result::Result<engine::BookInfo, String>);

/// The book that is open.
struct Open {
    book: String,
    view: Option<engine::View>,
    dirty: bool,
    /// The search bar of the lists (the text typed; the bar is drawn by the book's texture and the engine).
    search: horizon_client::ui::EditBox,
    /// `GuiBook.ticksInBook`: counts while Shift is not held (the stacks of an ingredient take turns).
    ticks: f32,
    /// Game ticks since the view was built.
    age: f32,
    last: Instant,
    /// Hover animation of the buttons (`timeHovered`, 0..5), by place.
    anim: HashMap<(i32, i32), f32>,
    /// The book list, when a guide book without a book was used.
    library: Option<usize>,
}

#[derive(Default)]
struct St {
    discovered: bool,
    lang: String,
    books: Vec<Listed>,
    loaded: HashMap<String, engine::Book>,
    navs: HashMap<String, engine::Nav>,
    persist: Option<engine::Persist>,
    open: Option<Open>,
    last_flip: Option<Instant>,
    /// Texture resource ids to the sprite of the GUI atlas they are (or `None`).
    sprites: HashMap<String, Option<String>>,
    /// The items that are a book (`custom_book_item`) to the book they open.
    custom_items: HashMap<String, String>,
}
thread_local! {
    static ST: RefCell<St> = RefCell::new(St::default());
}
fn with<R>(f: impl FnOnce(&mut St) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}
/// Like `with`, but gives up when the state is borrowed already (a hook re-entered).
fn try_with<R>(f: impl FnOnce(&mut St) -> R) -> Option<R> {
    ST.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

// ------------------------------------------------------------------------------------------------
// The game as the engine sees it
// ------------------------------------------------------------------------------------------------

struct GameEnv<'a, 'g> {
    g: &'a Game<'g>,
}
impl engine::Env for GameEnv<'_, '_> {
    fn lang_code(&self) -> String {
        horizon_client::lang::code()
    }
    fn translate(&self, key: &str) -> Option<String> {
        if key.is_empty() {
            return None;
        }
        if horizon_client::lang::has(key) {
            return Some(horizon_client::lang::get(key));
        }
        self.g.pack.names.get(key).map(str::to_owned)
    }
    fn item_exists(&self, id: &str) -> bool {
        self.g.res.items().get(id).is_some()
    }
    fn item_name(&self, stack: &engine::Stack) -> String {
        self.g.pack.names.display_item(&stack.id)
    }
    fn tag_items(&self, tag: &str) -> Vec<String> {
        let tag = tag.trim_start_matches('#');
        self.g.res.items().item_tag_order.get(tag).cloned().unwrap_or_default()
    }
    fn advancement_done(&self, id: &str) -> bool {
        let tree = &self.g.adv.tree;
        tree.by_id.get(id).is_some_and(|&i| self.g.adv.progress.done(&tree.list[i]))
    }
    fn mod_loaded(&self, ns: &str) -> bool {
        ns == "minecraft" || self.g.options.resources.join("assets").join(ns).is_dir() || self.g.options.resources.join("data").join(ns).is_dir()
    }
    fn recipe(&self, id: &str) -> Option<serde_json::Value> {
        let (ns, path) = id.split_once(':')?;
        let file = self.g.options.resources.join("data").join(ns).join("recipe").join(format!("{path}.json"));
        serde_json::from_slice(&std::fs::read(file).ok()?).ok()
    }
    fn advance(&self, c: char, bold: bool, uniform: bool) -> f32 {
        let style = TStyle { bold: Some(bold), font: uniform.then(|| UNIFORM.to_owned()), ..TStyle::default() };
        self.g.gui.atlas.font.advance(c, &style)
    }
    fn player_name(&self) -> String {
        self.g.options.player_name()
    }
    fn keybind(&self, name: &str) -> Option<(String, String)> {
        for mapping in [name.to_owned(), format!("key.{name}")] {
            let key = self.g.settings.keys.get(&mapping);
            if key != "key.keyboard.unknown" {
                let action = horizon_client::lang::get_or(&mapping, &mapping);
                return Some((action, horizon_client::keybinds::display_name(key).plain()));
            }
        }
        None
    }
    fn advanced_tooltips(&self) -> bool {
        self.g.settings.file.bool("advancedItemTooltips")
    }
    fn entity_name(&self, id: &str) -> Option<String> {
        let (ns, path) = id.split_once(':')?;
        let key = format!("entity.{ns}.{}", path.replace('/', "."));
        horizon_client::lang::has(&key).then(|| horizon_client::lang::get(&key))
    }
}

fn stack_of(s: &engine::Stack) -> ItemStack {
    ItemStack::new(&s.id, s.count.max(1))
}

fn argb(c: u32) -> u32 {
    rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
}

impl Game<'_> {
    fn pt_env(&self) -> GameEnv<'_, '_> {
        GameEnv { g: self }
    }
    fn pt_root(&self) -> PathBuf {
        self.options.resources.clone()
    }

    // ---- the library of books ----------------------------------------------------------------

    /// Finds the books and reads the reading state, once per world and language.
    fn pt_prepare(&self) {
        let lang = horizon_client::lang::code();
        if with(|st| st.discovered && st.lang == lang) {
            return;
        }
        let root = self.pt_root();
        let persist_text = self.native_setting(ID, DATA_KEY);
        with(|st| {
            if st.lang != lang {
                st.loaded.clear();
            }
            st.lang = lang;
            st.books = engine::discover(&engine::FsFiles(root));
            st.discovered = true;
            if st.persist.is_none() {
                st.persist = Some(engine::Persist::load(&persist_text.unwrap_or_default()));
            }
            st.custom_items = st
                .books
                .iter()
                .filter_map(|(r, i)| Some((i.as_ref().ok()?.custom_item()?.id, r.id())))
                .collect();
        });
    }

    fn pt_save(&self) {
        let text = with(|st| {
            let p = st.persist.as_mut()?;
            if !p.dirty {
                return None;
            }
            p.dirty = false;
            Some(p.save())
        });
        if let Some(text) = text {
            self.native_set_setting(ID, DATA_KEY, &text);
        }
    }

    /// Loads a book's contents the first time it is opened.
    fn pt_load(&self, id: &str) -> bool {
        let info = with(|st| {
            if st.loaded.contains_key(id) {
                return None;
            }
            st.books.iter().find(|(r, _)| r.id() == id).and_then(|(_, i)| i.as_ref().ok().cloned())
        });
        if let Some(info) = info {
            let env = self.pt_env();
            let book = engine::load_book(&engine::FsFiles(self.pt_root()), &env, info);
            for e in book.contents.errors.iter().take(20) {
                log::warn!("patchouli: {}: {e}", book.info.id);
            }
            log::info!("patchouli: book {} loaded: {} categories, {} entries", book.info.id, book.contents.categories.len(), book.contents.entries.len());
            with(|st| {
                st.loaded.insert(id.to_owned(), book);
            });
        }
        with(|st| st.loaded.contains_key(id))
    }

    /// The books that can be opened, with their titles and number of entries (loads them).
    pub(super) fn patchouli_books(&mut self) -> Vec<(String, String, usize)> {
        self.pt_prepare();
        let ids: Vec<String> = with(|st| st.books.iter().filter(|(_, i)| i.is_ok()).map(|(r, _)| r.id()).collect());
        let mut out = Vec::new();
        for id in ids {
            if self.pt_load(&id) {
                let env = self.pt_env();
                with(|st| {
                    if let Some(b) = st.loaded.get(&id) {
                        out.push((id.clone(), b.info.title(&env), b.contents.entries.len()));
                    }
                });
            }
        }
        out
    }

    // ---- opening ---------------------------------------------------------------------------

    /// `PatchouliAPI.openBookGUI(book)` and `openBookEntry(book, entry, page)`: the book opens (at an entry). Other
    /// native mods call it with their book item's use (the book needs no item of its own: `book_id` is `ns:name`).
    pub(super) fn patchouli_open_book(&mut self, book_id: &str, entry: Option<&str>, page: usize) -> bool {
        if !self.native_window() || !self.native_on(ID) {
            return false;
        }
        self.pt_prepare();
        let book_id = engine::qualify("patchouli", book_id);
        if !self.pt_load(&book_id) {
            let msg = horizon_client::lang::get_or("patchouli.networking.open_book.failed", "Failed to open book %s").replace("%s", &book_id);
            self.native_chat(&msg);
            return false;
        }
        let env = self.pt_env();
        let ok = with(|st| {
            let book = st.loaded.get(&book_id)?;
            let persist = st.persist.get_or_insert_with(Default::default);
            let nav = st.navs.entry(book_id.clone()).or_default();
            match entry {
                Some(e) => engine::open_at_entry(&env, book, persist, nav, &engine::qualify(&book.info.ns, e), page),
                None => engine::check_valid(&env, book, nav),
            }
            let mut search = horizon_client::ui::EditBox::new(0.0, 0.0, 90.0, 12.0);
            search.max_length = 32;
            search.bordered = false;
            st.open = Some(Open {
                book: book_id.clone(),
                view: None,
                dirty: true,
                search,
                ticks: 0.0,
                age: 0.0,
                last: Instant::now(),
                anim: HashMap::new(),
                library: None,
            });
            Some(())
        })
        .is_some();
        if !ok {
            return false;
        }
        let sound = with(|st| st.loaded.get(&book_id).map(|b| b.info.open_sound.clone())).unwrap_or_else(|| "patchouli:book_open".into());
        let pitch = 0.7 + self.pt_random() * 0.4;
        self.audio.play_unshared(&sound, None, 1.0, pitch);
        self.pt_show();
        self.pt_save();
        true
    }

    /// The list of books (a guide book that is no book yet).
    fn pt_open_library(&mut self) -> bool {
        let list = self.patchouli_books();
        if list.is_empty() {
            self.native_chat("Patchouli: no books were found in the installed mods.");
            return false;
        }
        let mut search = horizon_client::ui::EditBox::new(0.0, 0.0, 90.0, 12.0);
        search.bordered = false;
        with(|st| {
            st.open = Some(Open {
                book: String::new(),
                view: None,
                dirty: true,
                search,
                ticks: 0.0,
                age: 0.0,
                last: Instant::now(),
                anim: HashMap::new(),
                library: Some(0),
            });
        });
        self.pt_show();
        true
    }

    fn pt_show(&mut self) {
        self.screen = Screen::Advancements;
        self.capture(false);
    }

    fn pt_close(&mut self) {
        let was = with(|st| st.open.take().is_some());
        self.pt_save();
        if was && self.screen == Screen::Advancements {
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }

    fn pt_is_open(&self) -> bool {
        if self.screen != Screen::Advancements {
            // Another screen took over (death, a menu): the book is closed.
            let _ = try_with(|st| st.open = None);
            return false;
        }
        try_with(|st| st.open.is_some()).unwrap_or(true)
    }

    fn pt_random(&self) -> f32 {
        ((self.started.elapsed().as_nanos() % 1000) as f32) / 1000.0
    }

    /// The book a stack stands for: a guide book that names one (`ItemModBook.getBook`), or the item that a mod made its
    /// book (`custom_book_item`).
    fn pt_book_of(&self, stack: &ItemStack) -> Option<String> {
        if &*stack.item == "patchouli:guide_book" {
            let root = stack.data().and_then(|d| d.other("minecraft:custom_data"));
            return root.and_then(|t| t.get(TAG_BOOK)).and_then(|t| horizon_client::save::text(Some(t)));
        }
        self.pt_prepare();
        with(|st| st.custom_items.get(&*stack.item).cloned())
    }

    /// `BookRightClickHandler.getHoveredEntry`: with a book in hand, the entry that explains the block under the crosshair:
    /// (book, entry, spread).
    fn pt_hovered(&self, stack: &ItemStack) -> Option<(String, String, usize)> {
        let book_id = self.pt_book_of(stack)?;
        if !self.pt_load(&book_id) {
            return None;
        }
        let hit = self.pick()?;
        let [x, y, z] = hit.pos;
        if !self.world.loaded(x, z) {
            return None;
        }
        let name = self.res.facts().get(self.world.block(x, y, z)).name.to_string();
        let env = self.pt_env();
        with(|st| {
            let book = st.loaded.get(&book_id)?;
            let (entry, spread) = book.contents.entry_for_stack(&name)?.clone();
            (!book.contents.entry_locked(&env, &book.info, &entry)).then_some((book_id.clone(), entry, spread))
        })
    }

    /// The use of an item that is a book: it opens (`ItemModBook.use`); with Shift and a block that a book entry explains
    /// under the crosshair it opens at that entry (`BookRightClickHandler.onRightClick`).
    pub(super) fn patchouli_use_item(&mut self, stack: &ItemStack) -> bool {
        if !self.native_window() || !self.native_on(ID) || self.screen != Screen::Playing {
            return false;
        }
        let is_guide = &*stack.item == "patchouli:guide_book";
        match self.pt_book_of(stack) {
            Some(id) => {
                let sneaking = self.has_shift_down() || self.player.as_ref().is_some_and(|p| p.shift_key_down);
                if sneaking && let Some((book, entry, spread)) = self.pt_hovered(stack) {
                    let _ = self.patchouli_open_book(&book, Some(&entry), spread * 2);
                } else {
                    let _ = self.patchouli_open_book(&id, None, 0);
                }
                true
            }
            // `item.patchouli.guide_book.undefined`: there is no book in it; the list of books helps.
            None if is_guide => {
                self.pt_open_library();
                true
            }
            None => false,
        }
    }

    /// `BookRightClickHandler.onRenderHUD`: the entry for the block aimed at, next to the crosshair.
    pub(super) fn patchouli_hud(&self, batch: &mut HudBatch) {
        if self.screen != Screen::Playing || self.ui.hide_gui || !self.native_window() || !self.native_on(ID) {
            return;
        }
        let Some(stack) = self.held() else { return };
        // Only a book in hand costs anything here.
        if &*stack.item != "patchouli:guide_book" {
            self.pt_prepare();
            if !with(|st| st.custom_items.contains_key(&*stack.item)) {
                return;
            }
        }
        let Some((book_id, entry_id, _)) = self.pt_hovered(stack) else { return };
        let env = self.pt_env();
        let shown = with(|st| {
            let book = st.loaded.get(&book_id)?;
            let e = book.contents.entries.get(&entry_id)?;
            Some((engine::Icon::from_str(&e.icon), engine::display_name(&env, &book.info, &e.name)))
        });
        let Some((icon, name)) = shown else { return };
        let (w, h) = (batch.width as i32, batch.height as i32);
        let (x, y) = (w / 2 + 3, h / 2 + 3);
        let _ = try_with(|st| self.pt_icon(st, batch, &icon, x, y, 1.0, 1.0));
        let start = batch.vertices.len();
        self.draw_item(batch, stack, (x + 8) as f32, (y + 8) as f32);
        batch.scale_since(start, (x + 8) as f32, (y + 8) as f32, 0.5, 0.5);
        self.pt_text(batch, &name, (x + 18) as f32, (y + 3) as f32, 0xFFFF_FFFF, 1.0, false, false, None);
        let sneaking = self.has_shift_down() || self.player.as_ref().is_some_and(|p| p.shift_key_down);
        let key = if sneaking { "patchouli.gui.lexicon.view" } else { "patchouli.gui.lexicon.sneak" };
        let hint = horizon_client::lang::get_or(key, if sneaking { "View entry" } else { "Sneak to view" });
        let italic = engine::Style { color: 0xBBBBBB, italic: true, ..engine::Style::default() };
        self.pt_text(batch, &hint, (x + 18) as f32, (y + 14) as f32, 0xFF00_0000, 0.75, false, false, Some(&italic));
    }

    /// A world opens: nothing is open, the books are read again.
    pub(super) fn patchouli_world_opened() {
        with(|st| {
            st.open = None;
            st.loaded.clear();
            st.navs.clear();
            st.discovered = false;
            st.sprites.clear();
            st.custom_items.clear();
        });
    }

    /// The world is saved: so is what was read.
    pub(super) fn patchouli_save(&mut self) {
        self.pt_save();
    }

    // ---- the commands ------------------------------------------------------------------------

    /// `/open-patchouli-book [<targets>] <book> [<entry> [<page>]]` (`OpenBookCommand`), and `/patchouli list|give|open`.
    pub(super) fn patchouli_command(&mut self, command: &str) -> bool {
        if !self.native_on(ID) || !self.native_window() {
            return false;
        }
        let words: Vec<&str> = command.split_whitespace().collect();
        let (first, rest) = match words.split_first() {
            Some((f, r)) => (*f, r),
            None => return false,
        };
        if first != "open-patchouli-book" && first != "patchouli" {
            return false;
        }
        if !(self.gamemaster() || self.allow_commands() || self.creative) {
            self.native_chat("Patchouli: this command needs the cheats.");
            return true;
        }
        let args: Vec<&str> = if first == "patchouli" { rest.to_vec() } else { std::iter::once("open").chain(rest.iter().copied()).collect() };
        match args.as_slice() {
            ["list"] => {
                let books = self.patchouli_books();
                if books.is_empty() {
                    self.native_chat("Patchouli: no books.");
                }
                for (id, title, n) in books {
                    self.native_chat(&format!("{id}: {title} ({n} entries)"));
                }
            }
            ["give", book] => {
                let id = engine::qualify("patchouli", book);
                self.click_command(&format!("give @s patchouli:guide_book[custom_data={{\"{TAG_BOOK}\":\"{id}\"}}]"));
            }
            ["open", rest @ ..] => {
                // The first word may be a target (`@s`, a name); a book id has a colon.
                let rest: Vec<&str> = match rest.first() {
                    Some(t) if !t.contains(':') && rest.len() > 1 => rest[1..].to_vec(),
                    _ => rest.to_vec(),
                };
                match rest.as_slice() {
                    [book] => {
                        self.patchouli_open_book(book, None, 0);
                    }
                    [book, entry] => {
                        self.patchouli_open_book(book, Some(entry), 0);
                    }
                    [book, entry, page] => {
                        self.patchouli_open_book(book, Some(entry), page.parse().unwrap_or(0));
                    }
                    _ => self.native_chat("Usage: /open-patchouli-book <book> [<entry> [<page>]]"),
                }
            }
            _ => self.native_chat("Usage: /patchouli list | give <book> | open <book> [<entry> [<page>]]"),
        }
        true
    }

    // ---- drawing -----------------------------------------------------------------------------

    /// The sprite of the GUI atlas for a texture resource id (`ns:textures/gui/x.png`): the carry puts
    /// `textures/gui/**` into `textures/block/gui/**`, which is where the atlas finds it.
    fn pt_sprite_name(&self, st: &mut St, texture: &str) -> Option<String> {
        if let Some(hit) = st.sprites.get(texture) {
            return hit.clone();
        }
        let (ns, path) = texture.split_once(':').unwrap_or(("minecraft", texture));
        let p = path.strip_prefix("textures/").unwrap_or(path);
        let p = p.strip_suffix(".png").unwrap_or(p);
        let mut candidates = vec![format!("{ns}:block/{p}"), format!("{ns}:{p}"), format!("{ns}:block/gui/{}", p.rsplit('/').next().unwrap_or(p))];
        if ns == "minecraft" {
            candidates.insert(0, p.to_owned());
        }
        let found = candidates.into_iter().find(|c| self.gui.atlas.sprites.contains_key(c));
        st.sprites.insert(texture.to_owned(), found.clone());
        found
    }

    #[allow(clippy::too_many_arguments)]
    fn pt_blit(&self, batch: &mut HudBatch, name: &str, x: i32, y: i32, w: i32, h: i32, src: [i32; 4], tex: [i32; 2], alpha: f32) {
        let Some(r) = self.gui.atlas.sprites.get(name).copied() else { return };
        let [u, v, sw, sh] = src;
        let fu = |p: i32| r.u0 + (r.u1 - r.u0) * p as f32 / tex[0] as f32;
        let fv = |p: i32| r.v0 + (r.v1 - r.v0) * p as f32 / tex[1] as f32;
        let sub = Rect { u0: fu(u), v0: fv(v), u1: fu(u + sw), v1: fv(v + sh), w: sw as u32, h: sh as u32 };
        batch.rect_uv(x as f32, y as f32, w as f32, h as f32, sub, rgba(255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8));
    }

    fn pt_text(&self, batch: &mut HudBatch, s: &str, x: f32, y: f32, color: u32, scale: f32, uniform: bool, shadow: bool, extra: Option<&engine::Style>) -> f32 {
        let mut style = TStyle {
            color: Some(extra.map_or(color & 0xFF_FFFF, |e| e.color)),
            font: uniform.then(|| UNIFORM.to_owned()),
            ..TStyle::default()
        };
        if let Some(e) = extra {
            style.bold = e.bold.then_some(true);
            style.italic = e.italic.then_some(true);
            style.underlined = e.underline.then_some(true);
            style.strikethrough = e.strike.then_some(true);
            style.obfuscated = e.obf.then_some(true);
        }
        let text = Text::styled(s, style);
        let start = batch.vertices.len();
        let w = batch.text_styled(&self.gui.atlas, &text, x, y, rgba(255, 255, 255, (color >> 24) as u8), shadow);
        if scale != 1.0 {
            batch.scale_since(start, x, y, scale, scale);
        }
        w
    }

    /// The pixel size of a stack's tooltip place, and the stack if it exists in this game.
    fn pt_item(&self, s: &engine::Stack) -> Option<ItemStack> {
        self.res.items().get(&s.id).is_some().then(|| stack_of(s))
    }

    /// One frame of the open book. Returns what the cursor is on, for the tooltip.
    fn pt_paint(&self, st: &mut St, batch: &mut HudBatch) -> Hover {
        let mut hover = Hover::default();
        let Some(mut open) = st.open.take() else { return hover };
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width as i32, batch.height as i32);
        // renderBackground
        batch.fill_gradient(atlas, (0.0, 0.0, w as f32, h as f32), rgba(0x10, 0x10, 0x10, 0xC0), rgba(0x10, 0x10, 0x10, 0xD0));
        let (ox, oy) = (w / 2 - engine::FULL_WIDTH / 2, h / 2 - engine::FULL_HEIGHT / 2);
        // time
        let now = Instant::now();
        let dt = now.duration_since(open.last).as_secs_f32() * 20.0;
        open.last = now;
        if !self.has_shift_down() {
            open.ticks += dt;
        }
        open.age += dt;
        let (mx, my) = (f64::from(self.cursor.0) - f64::from(ox), f64::from(self.cursor.1) - f64::from(oy));
        let total = self.started.elapsed().as_secs_f32() * 20.0;
        // the view
        if open.dirty || open.view.is_none() || open.age > 40.0 {
            self.pt_rebuild(st, &mut open);
        }
        if let Some(view) = open.view.take() {
            let info = if open.library.is_some() { None } else { st.loaded.get(&open.book).map(|b| b.info.clone()) };
            for (idx, e) in view.elems.iter().enumerate() {
                self.pt_elem(st, batch, &mut open, info.as_ref(), e, idx, (ox, oy), (mx, my), (dt, total), &mut hover);
            }
            open.view = Some(view);
        }
        st.open = Some(open);
        hover
    }

    fn pt_rebuild(&self, st: &mut St, open: &mut Open) {
        let env = self.pt_env();
        if let Some(page) = open.library {
            let items: Vec<engine::LibraryItem> = st
                .books
                .iter()
                .filter_map(|(r, i)| i.as_ref().ok().map(|_| r.id()))
                .filter_map(|id| st.loaded.get(&id))
                .map(|b| engine::LibraryItem { id: b.info.id.clone(), title: b.info.title(&env), entries: b.contents.entries.len(), icon: b.info.custom_item().map_or_else(|| engine::Icon::Stack(engine::Stack::new("patchouli:guide_book", 1)), engine::Icon::Stack) })
                .collect();
            open.view = Some(engine::build_library(&env, &items, page));
        } else if let (Some(book), Some(persist)) = (st.loaded.get(&open.book), st.persist.as_ref()) {
            if let Some(nav) = st.navs.get_mut(&open.book) {
                engine::normalize(&env, book, persist, nav);
                if open.search.value != nav.search {
                    open.search.set_value(&nav.search);
                }
                open.view = Some(engine::build_view(&env, book, persist, nav));
            }
        }
        open.dirty = false;
        open.age = 0.0;
    }

    #[allow(clippy::too_many_arguments)]
    fn pt_elem(
        &self,
        st: &mut St,
        batch: &mut HudBatch,
        open: &mut Open,
        info: Option<&engine::BookInfo>,
        e: &engine::Elem,
        idx: usize,
        (ox, oy): (i32, i32),
        (mx, my): (f64, f64),
        (dt, total): (f32, f32),
        hover: &mut Hover,
    ) {
        use engine::Elem;
        let atlas = &self.gui.atlas;
        let inside = |x: i32, y: i32, w: i32, h: i32| mx >= f64::from(x) && my >= f64::from(y) && mx < f64::from(x + w) && my < f64::from(y + h);
        let uniform = info.is_none_or(|i| !i.use_blocky_font);
        let book_tex = info.map_or("patchouli:textures/gui/book_brown.png".to_owned(), |i| i.book_texture.clone());
        let tex_name = |this: &Self, st: &mut St, t: engine::Tex| -> Option<String> {
            let wanted = match t {
                engine::Tex::Book => book_tex.clone(),
                engine::Tex::Crafting => info.map_or("patchouli:textures/gui/crafting.png".to_owned(), |i| i.crafting_texture.clone()),
                engine::Tex::Filler => info.map_or("patchouli:textures/gui/page_filler.png".to_owned(), |i| i.filler_texture.clone()),
            };
            let default = match t {
                engine::Tex::Book => "patchouli:textures/gui/book_brown.png",
                engine::Tex::Crafting => "patchouli:textures/gui/crafting.png",
                engine::Tex::Filler => "patchouli:textures/gui/page_filler.png",
            };
            this.pt_sprite_name(st, &wanted).or_else(|| this.pt_sprite_name(st, default))
        };
        match e {
            Elem::Fill { rect: [x1, y1, x2, y2], argb: c } => {
                batch.fill(atlas, (ox + x1) as f32, (oy + y1) as f32, (x2 - x1) as f32, (y2 - y1) as f32, argb(*c));
            }
            Elem::Gradient { rect: [x1, y1, x2, y2], top, bottom } => {
                batch.fill_gradient(atlas, ((ox + x1) as f32, (oy + y1) as f32, (x2 - x1) as f32, (y2 - y1) as f32), argb(*top), argb(*bottom));
            }
            Elem::Blit { tex, x, y, w, h, u, v, sw, sh, tw, th, alpha } => match tex_name(self, st, *tex) {
                Some(name) => self.pt_blit(batch, &name, ox + x, oy + y, *w, *h, [*u, *v, *sw, *sh], [*tw, *th], *alpha),
                None => {
                    // No texture: a plain page, so the book can still be read.
                    if *tex == engine::Tex::Book && *u == 0 && *v == 0 {
                        batch.fill(atlas, (ox + x) as f32, (oy + y) as f32, *w as f32, *h as f32, rgba(0xE8, 0xD8, 0xB0, 255));
                    }
                }
            },
            Elem::Image { texture, x, y, w, h, u, v, sw, sh, tw, th } => {
                if let Some(name) = self.pt_sprite_name(st, texture) {
                    self.pt_blit(batch, &name, ox + x, oy + y, *w, *h, [*u, *v, *sw, *sh], [*tw, *th], 1.0);
                } else {
                    batch.fill(atlas, (ox + x) as f32, (oy + y) as f32, *w as f32, *h as f32, rgba(0, 0, 0, 40));
                }
            }
            Elem::Str { text, x, y, color, scale, uniform: u, shadow } => {
                self.pt_text(batch, text, (ox + x) as f32, (oy + y) as f32, 0xFF00_0000 | color, *scale, *u, *shadow, None);
            }
            Elem::Words { text, uniform: u } => {
                let start = batch.vertices.len();
                for (i, word) in text.words.iter().enumerate() {
                    let hot = text.is_hot(i, mx, my);
                    let mut style = word.style;
                    if hot && word.link.is_some() {
                        style.color = info.map_or(0x8800EE, |b| b.link_hover_color);
                    }
                    if hot && !word.tooltip.is_empty() && hover.tip.is_empty() {
                        hover.tip = vec![word.tooltip.clone()];
                    }
                    self.pt_text(batch, &word.text, (ox + word.x) as f32, (oy + word.y) as f32, 0xFF00_0000, 1.0, *u, false, Some(&style));
                }
                if text.scale != 1.0
                    && let Some(first) = text.words.first()
                {
                    batch.scale_since(start, (ox + first.x) as f32, (oy + first.y) as f32, text.scale, text.scale);
                }
            }
            Elem::Item { stacks, x, y, scale } => {
                if stacks.is_empty() {
                    return;
                }
                let s = &stacks[(open.ticks / 20.0) as usize % stacks.len()];
                let (px, py) = (ox + x, oy + y);
                match self.pt_item(s) {
                    Some(item) => {
                        let start = batch.vertices.len();
                        self.draw_item(batch, &item, px as f32, py as f32);
                        if *scale != 1.0 {
                            batch.scale_since(start, px as f32, py as f32, *scale, *scale);
                        }
                    }
                    None => {
                        let size = (16.0 * scale) as f32;
                        batch.fill(atlas, px as f32, py as f32, size, size, rgba(0x80, 0x80, 0x80, 90));
                        self.pt_text(batch, "?", px as f32 + size / 2.0 - 2.0, py as f32 + size / 2.0 - 4.0, 0xFF55_5555, 1.0, false, false, None);
                    }
                }
                if inside(*x, *y, (16.0 * scale) as i32, (16.0 * scale) as i32) {
                    hover.item = Some(s.clone());
                }
            }
            Elem::Icon { icon, x, y, scale, alpha } => self.pt_icon(st, batch, icon, ox + x, oy + y, *scale, *alpha),
            Elem::Button { rect: [x, y, w, h], u, v, tip, .. } => {
                let hot = inside(*x, *y, *w, *h);
                if let Some(name) = tex_name(self, st, engine::Tex::Book) {
                    self.pt_blit(batch, &name, ox + x, oy + y, *w, *h, [u + if hot { *w } else { 0 }, *v, *w, *h], [512, 256], 1.0);
                }
                if hot {
                    hover.tip = tip.clone();
                }
            }
            Elem::EntryBtn { x, y, label, locked, priority, argb: c, icon, state, .. } => {
                let (w, h) = (engine::PAGE_WIDTH, 10);
                let hot = inside(*x, *y, w, h);
                let t = open.anim.entry((*x, *y)).or_insert(0.0);
                *t = if hot { (*t + dt).min(5.0) } else { (*t - dt).max(0.0) };
                let frac = *t / 5.0;
                if frac > 0.0 {
                    batch.fill(atlas, (ox + x) as f32, (oy + y) as f32, (w as f32 * frac).floor(), h as f32, rgba(0, 0, 0, 0x22));
                }
                if *locked {
                    if let Some(name) = tex_name(self, st, engine::Tex::Book) {
                        self.pt_blit(batch, &name, ox + x + 1, oy + y + 1, 8, 8, [250, 180, 16, 16], [512, 256], 0.7);
                    }
                } else {
                    self.pt_icon(st, batch, icon, ox + x + 1, oy + y + 1, 0.5, 1.0);
                }
                let mut style = engine::Style { color: *c & 0xFF_FFFF, ..engine::Style::default() };
                style.italic = *priority;
                self.pt_text(batch, label, (ox + x + 12) as f32, (oy + y) as f32, *c, 1.0, uniform, false, Some(&style));
                if !*locked {
                    self.pt_marking(st, batch, info, ox + x + w - 5, oy + y + 1, *state, total);
                }
            }
            Elem::CatBtn { x, y, name, icon, locked, state, .. } => {
                let hot = inside(*x, *y, 20, 20);
                let t = open.anim.entry((*x, *y)).or_insert(0.0);
                *t = if hot { (*t + dt).min(5.0) } else { (*t - dt).max(0.0) };
                let veil = 0.5 - (*t / 5.0) * 0.5;
                if *locked {
                    if let Some(n) = tex_name(self, st, engine::Tex::Book) {
                        self.pt_blit(batch, &n, ox + x + 2, oy + y + 2, 16, 16, [250, 180, 16, 16], [512, 256], 0.7);
                    }
                } else {
                    self.pt_icon(st, batch, icon, ox + x + 2, oy + y + 2, 1.0, 1.0);
                }
                // The page's own texture over the icon makes it look faded until hovered.
                if let Some(n) = tex_name(self, st, engine::Tex::Book) {
                    self.pt_blit(batch, &n, ox + x, oy + y, 20, 20, [*x, *y, 20, 20], [512, 256], veil);
                }
                if let Some(s) = state {
                    self.pt_marking(st, batch, info, ox + x, oy + y, *s, total);
                }
                if hot {
                    hover.tip = vec![if *locked { "§7(Locked)".to_owned() } else { name.clone() }];
                    if *locked {
                        let l = horizon_client::lang::get_or("patchouli.gui.lexicon.locked", "(Locked)");
                        hover.tip = vec![format!("§7{l}")];
                    }
                }
            }
            Elem::Bookmark { x, y, tip, icon, number, kind, .. } => {
                let hot = inside(*x, *y, 13, 10);
                if let Some(n) = tex_name(self, st, engine::Tex::Book) {
                    self.pt_blit(batch, &n, ox + x, oy + y, 13, 10, [272 + if hot { 13 } else { 0 }, *kind, 13, 10], [512, 256], 1.0);
                }
                if let Some(ic) = icon {
                    let px = ox + x + if hot { 3 } else { 1 };
                    self.pt_icon(st, batch, ic, px, oy + y + 1, 0.5, 1.0);
                    self.pt_text(batch, number, (px + 6) as f32, (oy + y + 6) as f32, 0xFFFF_FFFF, 0.5, false, true, None);
                }
                if hot {
                    hover.tip = tip.clone();
                }
            }
            Elem::MarkRead { x, y, tip, .. } => {
                let hot = inside(*x, *y, 11, 11);
                if let Some(n) = tex_name(self, st, engine::Tex::Book) {
                    self.pt_blit(batch, &n, ox + x, oy + y, 13, 10, [285, 160, 13, 10], [512, 256], 1.0);
                    self.pt_blit(batch, &n, ox + x + 1, oy + y, 11, 11, [308 + if hot { 11 } else { 0 }, 31, 11, 11], [512, 256], 1.0);
                }
                self.pt_text(batch, "+", (ox + x + 1) as f32, (oy + y) as f32, 0xFF00_FF01, 1.0, false, true, None);
                if hot {
                    hover.tip = vec![tip.clone()];
                }
            }
            Elem::PlainButton { rect: [x, y, w, h], text, .. } => {
                let hot = inside(*x, *y, *w, *h);
                let sprite = if hot { "gui/sprites/widget/button_highlighted" } else { "gui/sprites/widget/button" };
                horizon_client::ui::nine_slice(batch, atlas, sprite, (ox + x) as f32, (oy + y) as f32, *w as f32, *h as f32, u32::MAX);
                let tw = HudBatch::text_width(atlas, text);
                batch.text(atlas, text, (ox + x) as f32 + (*w as f32 - tw) / 2.0, (oy + y) as f32 + (*h as f32 - 8.0) / 2.0, u32::MAX, true);
            }
            Elem::Tip { rect: [x, y, w, h], lines } => {
                if inside(*x, *y, *w, *h) && hover.tip.is_empty() {
                    hover.tip = lines.clone();
                }
            }
            Elem::Entity { id, x, y, render_size, extra_offset, rotate, default_rotation } => {
                self.pt_entity(batch, id, ox + x, oy + y, *render_size, *extra_offset, if *rotate { total * 3.0 } else { *default_rotation });
            }
            Elem::Blocks { blocks, size, x, y, room } => {
                self.pt_blocks(batch, blocks, *size, ox + x, oy + y, *room, total);
            }
        }
        let _ = idx;
    }

    fn pt_icon(&self, st: &mut St, batch: &mut HudBatch, icon: &engine::Icon, x: i32, y: i32, scale: f32, alpha: f32) {
        match icon {
            engine::Icon::Stack(s) => {
                if let Some(item) = self.pt_item(s) {
                    let start = batch.vertices.len();
                    self.draw_item(batch, &item, x as f32, y as f32);
                    if scale != 1.0 {
                        batch.scale_since(start, x as f32, y as f32, scale, scale);
                    }
                }
            }
            engine::Icon::Texture(t) => {
                if let Some(name) = self.pt_sprite_name(st, t) {
                    let size = (16.0 * scale) as i32;
                    self.pt_blit(batch, &name, x, y, size, size, [0, 0, 16, 16], [16, 16], alpha);
                }
            }
        }
    }

    /// `GuiBook.drawMarking`.
    fn pt_marking(&self, st: &mut St, batch: &mut HudBatch, info: Option<&engine::BookInfo>, x: i32, y: i32, state: engine::ReadState, total: f32) {
        if !state.has_icon() {
            return;
        }
        let _ = info;
        let alpha = if state.has_animation() { (total * 0.2).sin() * 0.3 + 0.7 } else { 1.0 };
        let book_tex = with_book_tex(st, self, info);
        if let Some(name) = book_tex {
            self.pt_blit(batch, &name, x, y, 8, 8, [state.u(), 197, 8, 8], [512, 256], alpha);
        }
    }

    /// `PageEntity.renderEntity`: a mob turned about, drawn like the preview of a mount's menu.
    #[allow(clippy::too_many_arguments)]
    fn pt_entity(&self, batch: &mut HudBatch, id: &str, cx: i32, cy: i32, render_size: f32, extra_offset: f32, rotation: f32) {
        let Some(kind) = self.res.mob_types().get(id).cloned() else {
            self.pt_text(batch, &horizon_client::lang::get_or("patchouli.gui.lexicon.loading_error", "Loading error!"), (cx - 30) as f32, cy as f32, 0xFFFF_0000, 1.0, false, true, None);
            return;
        };
        let (width, height) = (kind.width as f32, kind.height as f32);
        let entity_size = width.max(height).max(1.0);
        let scale = render_size / entity_size * 0.8;
        let offset = height.max(entity_size) * 0.5 + extra_offset;
        let mut m = horizon_client::mobs::Mob::new(0, kind, horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0), 0.0);
        m.previous = m.position;
        m.body_yaw = rotation;
        m.previous_body_yaw = m.body_yaw;
        m.head_yaw = rotation;
        m.previous_head_yaw = rotation;
        m.head_pitch = 0.0;
        m.previous_head_pitch = 0.0;
        let atlas = &self.gui.atlas;
        let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
        let mut verts = Vec::new();
        horizon_client::mobs::render::render_with(&m, 1.0, glam::DVec3::new(m.position.x, m.position.y, m.position.z), &lookup, 240 | 240 << 8, self.res.mob_data(), &[], &mut verts);
        let gui = glam::Mat4::from_translation(glam::Vec3::new(cx as f32, cy as f32, 0.0))
            * glam::Mat4::from_scale(glam::Vec3::new(scale, scale, -scale))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, offset, 0.0))
            * glam::Mat4::from_rotation_z(std::f32::consts::PI);
        let mut quads: Vec<(f32, [[f32; 2]; 4], [[f32; 2]; 4], u32)> = Vec::new();
        for q in verts.as_chunks::<4>().0 {
            let p: [glam::Vec3; 4] = std::array::from_fn(|i| gui.transform_point3(glam::Vec3::from_array(q[i].pos)));
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            if n.z >= 0.0 {
                continue;
            }
            let n = n.normalize_or_zero();
            let shade = (0.55 + 0.45 * -n.z - 0.15 * n.y).clamp(0.4, 1.0);
            let c = q[0].color;
            let ch = |s: u32| ((((c >> s) & 255) as f32 * shade) as u32) << s;
            let color = (c & 0xFF00_0000) | ch(0) | ch(8) | ch(16);
            let depth = p.iter().map(|v| v.z).sum::<f32>() / 4.0;
            quads.push((depth, p.map(|v| [v.x, v.y]), std::array::from_fn(|i| q[i].uv), color));
        }
        quads.sort_by(|a, b| a.0.total_cmp(&b.0));
        let start = batch.vertices.len();
        for (_, p, uv, color) in quads {
            batch.quad(p, uv, color);
        }
        batch.clip_since(start, (cx - 53) as f32, (cy - 53) as f32, (cx + 53) as f32, (cy + 46) as f32);
    }

    /// A multiblock from the icons of its blocks, turned about the vertical axis in quarter turns.
    #[allow(clippy::too_many_arguments)]
    fn pt_blocks(&self, batch: &mut HudBatch, blocks: &[(i32, i32, i32, String)], size: [i32; 3], cx: i32, cy: i32, room: i32, total: f32) {
        if blocks.is_empty() {
            return;
        }
        let turn = ((total / 50.0) as i32).rem_euclid(4);
        let [sx, sy, sz] = size;
        // Isometric: x and z go down diagonally, y goes up. One block takes `step` pixels.
        let rotate = |x: i32, z: i32| match turn {
            0 => (x, z),
            1 => (sz - 1 - z, x),
            2 => (sx - 1 - x, sz - 1 - z),
            _ => (z, sx - 1 - x),
        };
        let (w, d) = if turn % 2 == 0 { (sx, sz) } else { (sz, sx) };
        let span_x = (w + d) as f32;
        let span_y = (w + d) as f32 / 2.0 + sy as f32;
        let step = (room as f32 / span_x.max(span_y)).clamp(3.0, 16.0);
        let mut ordered: Vec<(i32, i32, i32, &str)> = blocks.iter().map(|(x, y, z, id)| (x - 0, *y, *z, id.as_str())).collect();
        ordered.sort_by_key(|&(x, y, z, _)| {
            let (rx, rz) = rotate(x, z);
            (y, rx + rz)
        });
        let start = batch.vertices.len();
        for (x, y, z, id) in ordered {
            let (rx, rz) = rotate(x, z);
            let px = cx as f32 + (rx - rz) as f32 * step / 2.0 - step / 2.0;
            let py = cy as f32 + (rx + rz) as f32 * step / 4.0 - y as f32 * step / 2.0 + (sy as f32 * step / 4.0) - span_y * step / 4.0;
            if let Some(item) = self.pt_item(&engine::Stack::new(id, 1)) {
                let s = batch.vertices.len();
                self.draw_item_model(batch, &item, px, py);
                batch.scale_since(s, px, py, step / 16.0, step / 16.0);
            }
        }
        batch.clip_since(start, (cx - 53) as f32, (cy - 53) as f32, (cx + 53) as f32, (cy + 46) as f32);
    }
}

fn with_book_tex(st: &mut St, g: &Game<'_>, info: Option<&engine::BookInfo>) -> Option<String> {
    let wanted = info.map_or("patchouli:textures/gui/book_brown.png".to_owned(), |i| i.book_texture.clone());
    g.pt_sprite_name(st, &wanted).or_else(|| g.pt_sprite_name(st, "patchouli:textures/gui/book_brown.png"))
}

/// What the pointer is on, after a frame.
#[derive(Default)]
struct Hover {
    tip: Vec<String>,
    item: Option<engine::Stack>,
}

// ------------------------------------------------------------------------------------------------
// Input
// ------------------------------------------------------------------------------------------------

impl Game<'_> {
    /// HEAD on `draw_advancements`: the book, in place of the advancements.
    pub(super) fn patchouli_draw(&mut self, batch: &mut HudBatch) -> bool {
        if !self.pt_is_open() {
            return false;
        }
        let me: &Game<'_> = self;
        let hover = match try_with(|st| me.pt_paint(st, batch)) {
            Some(h) => h,
            None => return true,
        };
        let (x, y) = self.cursor;
        if let Some(stack) = hover.item {
            if let Some(item) = self.pt_item(&stack) {
                let mut lines = self.stack_tooltip(&item);
                // `GuiBook.drawTooltip`: "(Shift-Click for Recipe)" when another entry explains the item.
                let provider = with(|st| {
                    let o = st.open.as_ref()?;
                    let nav = st.navs.get(&o.book)?;
                    let book = st.loaded.get(&o.book)?;
                    let (entry, _) = book.contents.entry_for_stack(&stack.id)?.clone();
                    (!matches!(&nav.cur.gui, engine::Gui::Entry(e) if *e == entry)).then_some(entry)
                });
                if provider.is_some() {
                    let label = horizon_client::lang::get_or("patchouli.gui.lexicon.shift_for_recipe", "Shift-Click for Recipe");
                    lines.push((Text::legacy(&format!("§6({label})")), u32::MAX));
                }
                self.draw_tooltip_lines(batch, &lines, x, y);
            } else {
                self.draw_tooltip_lines(batch, &[(Text::legacy(&stack.id), u32::MAX)], x, y);
            }
        } else if !hover.tip.is_empty() {
            let lines: Vec<(Text, u32)> = hover.tip.iter().map(|l| (Text::legacy(l), u32::MAX)).collect();
            self.draw_tooltip_lines(batch, &lines, x, y);
        }
        true
    }

    /// `GuiBook.playBookFlipSound`: at most one every 6 ticks.
    fn pt_flip(&mut self) {
        let now = Instant::now();
        let (play, sound) = with(|st| {
            let recent = st.last_flip.is_some_and(|t| now.duration_since(t).as_millis() <= 300);
            if !recent {
                st.last_flip = Some(now);
            }
            let sound = st
                .open
                .as_ref()
                .and_then(|o| st.loaded.get(&o.book))
                .map_or("patchouli:book_flip".to_owned(), |b| b.info.flip_sound.clone());
            (!recent, sound)
        });
        if play {
            let pitch = 0.7 + self.pt_random() * 0.3;
            self.audio.play_unshared(&sound, None, 1.0, pitch);
        }
    }

    fn pt_effects(&mut self, fx: Vec<engine::Effect>) {
        for e in fx {
            match e {
                engine::Effect::Flip => self.pt_flip(),
                engine::Effect::OpenUrl(u) => self.link_prompt = Some(u),
                engine::Effect::Command(c) => {
                    self.pt_close();
                    self.click_command(&c);
                }
                engine::Effect::Save => self.pt_save(),
                engine::Effect::Close => self.pt_close(),
            }
        }
    }

    /// HEAD on `sort_mouse_press` (a press of any button while the book is open).
    pub(super) fn patchouli_press(&mut self, button: winit::event::MouseButton) -> bool {
        use winit::event::MouseButton as B;
        if !self.pt_is_open() {
            return false;
        }
        let (w, h) = self
            .renderer
            .as_ref()
            .map_or((1.0, 1.0), |r| {
                let b = HudBatch::new([r.config.width, r.config.height]);
                (b.width, b.height)
            });
        let (ox, oy) = (w as i32 / 2 - engine::FULL_WIDTH / 2, h as i32 / 2 - engine::FULL_HEIGHT / 2);
        let (mx, my) = (f64::from(self.cursor.0) - f64::from(ox), f64::from(self.cursor.1) - f64::from(oy));
        let shift = self.has_shift_down();
        let env_fx = {
            let me: &Game<'_> = self;
            try_with(|st| me.pt_press(st, button, (mx, my), shift))
        };
        let Some(action) = env_fx else { return true };
        match action {
            Pressed::Fx(fx) => {
                with(|st| {
                    if let Some(o) = st.open.as_mut() {
                        o.dirty = true;
                    }
                });
                self.pt_effects(fx);
            }
            Pressed::OpenBook(id) => {
                self.pt_close();
                self.patchouli_open_book(&id, None, 0);
            }
            Pressed::Nothing => {}
        }
        let _ = B::Left;
        true
    }

    fn pt_press(&self, st: &mut St, button: winit::event::MouseButton, (mx, my): (f64, f64), shift: bool) -> Pressed {
        use winit::event::MouseButton as B;
        let Some(open) = st.open.as_mut() else { return Pressed::Nothing };
        let env = self.pt_env();
        // The list of books
        if let Some(page) = open.library {
            if button != B::Left {
                return Pressed::Nothing;
            }
            let hit = open.view.as_ref().and_then(|v| hit_cmd(v, mx, my));
            match hit {
                Some(engine::Cmd::OpenBook(id)) => return Pressed::OpenBook(id),
                Some(engine::Cmd::Page(left)) => {
                    open.library = Some(if left { page.saturating_sub(1) } else { page + 1 });
                    open.dirty = true;
                    return Pressed::Fx(vec![engine::Effect::Flip]);
                }
                _ => return Pressed::Nothing,
            }
        }
        let id = open.book.clone();
        let (Some(book), Some(nav)) = (st.loaded.get(&id), st.navs.get_mut(&id)) else { return Pressed::Nothing };
        let persist = st.persist.get_or_insert_with(Default::default);
        let view = match open.view.as_ref() {
            Some(v) => v,
            None => return Pressed::Nothing,
        };
        let mut fx = Vec::new();
        match button {
            B::Left => {
                if shift && let Some((stack, _)) = hover_stack(view, open, mx, my) {
                    fx = engine::open_for_stack(&env, book, persist, nav, &stack.id);
                    if !fx.is_empty() {
                        return Pressed::Fx(fx);
                    }
                }
                // The text first, then the buttons (as `mouseClickedScaled`).
                let mut cmd = None;
                for e in view.elems.iter().rev() {
                    if let engine::Elem::Words { text, .. } = e
                        && let Some(link) = text.click(mx, my, 0)
                    {
                        cmd = Some(engine::Cmd::Link(link));
                        break;
                    }
                }
                if cmd.is_none() {
                    cmd = hit_cmd(view, mx, my);
                }
                match cmd {
                    Some(engine::Cmd::OpenBook(b)) => return Pressed::OpenBook(b),
                    Some(c) => fx = engine::apply_cmd(&env, book, persist, nav, view, &c, shift),
                    None => {}
                }
            }
            B::Right => {
                if engine::back(&env, book, persist, nav, shift) {
                    fx.push(engine::Effect::Flip);
                }
            }
            B::Back => {
                if engine::change_page(view, nav, true) {
                    fx.push(engine::Effect::Flip);
                }
            }
            B::Forward => {
                if engine::change_page(view, nav, false) {
                    fx.push(engine::Effect::Flip);
                }
            }
            _ => {}
        }
        if persist.dirty {
            fx.push(engine::Effect::Save);
        }
        Pressed::Fx(fx)
    }

    /// HEAD on `advancements_scroll`: the wheel turns the pages.
    pub(super) fn patchouli_scroll(&mut self, delta: f32) -> bool {
        if !self.pt_is_open() {
            return false;
        }
        let left = delta > 0.0;
        if delta == 0.0 {
            return true;
        }
        let changed = try_with(|st| {
            let o = st.open.as_mut()?;
            if let Some(page) = o.library {
                let max = o.view.as_ref().map_or(1, |v| v.max_spreads);
                let next = if left { page.saturating_sub(1) } else { (page + 1).min(max.saturating_sub(1)) };
                o.library = Some(next);
                o.dirty = true;
                return Some(next != page);
            }
            let view = o.view.as_ref()?;
            let nav = st.navs.get_mut(&o.book)?;
            let did = engine::change_page(view, nav, left);
            o.dirty |= did;
            Some(did)
        })
        .flatten()
        .unwrap_or(false);
        if changed {
            self.pt_flip();
        }
        true
    }

    /// HEAD on `advancements_mouse`: the release of the left button (the press was `sort_mouse_press`'s).
    pub(super) fn patchouli_release(&mut self) -> bool {
        self.pt_is_open()
    }

    /// HEAD on `sort_key_press`: Escape closes, Backspace goes back; every other key but the screenshot, fullscreen and
    /// hide-GUI keys belongs to the book.
    pub(super) fn patchouli_key(&mut self, key: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode as K;
        if !self.pt_is_open() {
            return false;
        }
        // `GuiBook.keyPressed`: the inventory key closes a book that has no way back; `GuiBookEntry` always closes. (key_pressed
        // gets the default key of the mapping, so the inventory key is E whatever it is bound to.)
        let closes_with_inventory = try_with(|st| {
            let Some(o) = st.open.as_ref() else { return true };
            if o.library.is_some() {
                return true;
            }
            match st.navs.get(&o.book) {
                Some(n) => matches!(n.cur.gui, engine::Gui::Entry(_)) || n.stack.is_empty(),
                None => true,
            }
        })
        .unwrap_or(true);
        if key == K::Escape || (key == K::KeyE && closes_with_inventory) {
            self.pt_close();
            return true;
        }
        if key == K::Backspace {
            let shift = self.has_shift_down();
            let did = {
                let me: &Game<'_> = self;
                try_with(|st| {
                    let o = st.open.as_mut()?;
                    let id = o.book.clone();
                    let (book, nav) = (st.loaded.get(&id)?, st.navs.get_mut(&id)?);
                    let persist = st.persist.get_or_insert_with(Default::default);
                    let did = engine::back(&me.pt_env(), book, persist, nav, shift);
                    o.dirty |= did;
                    Some(did)
                })
                .flatten()
                .unwrap_or(false)
            };
            if did {
                self.pt_flip();
            }
            return true;
        }
        !matches!(key, K::F1 | K::F2 | K::F11)
    }

    /// HEAD on `recipe_search_focused`: the lists take typed keys.
    pub(super) fn patchouli_typing(&self) -> bool {
        self.pt_is_open() && try_with(|st| st.open.as_ref().is_some_and(|o| o.view.as_ref().is_some_and(|v| v.searchable))).unwrap_or(false)
    }

    /// HEAD on `recipe_key`: a key typed into the search bar (`charTyped` and `keyPressed` of the `EditBox`).
    pub(super) fn patchouli_type(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode as K, PhysicalKey};
        if !self.patchouli_typing() {
            return false;
        }
        let shortcut = self.edit_shortcut(event);
        let (ctrl, shift) = (self.has_control_down(), self.has_shift_down());
        let clipboard = &mut self.clipboard;
        let enter = matches!(event.physical_key, PhysicalKey::Code(K::Enter | K::NumpadEnter));
        let opened = try_with(|st| {
            let o = st.open.as_mut()?;
            if enter {
                return Some(o.view.as_ref().and_then(|v| v.single_result.clone()));
            }
            if let Some(sc) = shortcut {
                o.search.shortcut(sc, clipboard);
            } else {
                match event.physical_key {
                    PhysicalKey::Code(code) if o.search.key(code, ctrl, shift) => {}
                    _ => {
                        if !ctrl && let Some(text) = &event.text {
                            let text: String = text.chars().filter(|c| !c.is_control()).collect();
                            o.search.insert_text(&text);
                        }
                    }
                }
            }
            o.search.reset_blink();
            let value = o.search.value.clone();
            let id = o.book.clone();
            if let Some(nav) = st.navs.get_mut(&id) {
                nav.search = value;
                o.dirty = true;
            }
            Some(None)
        })
        .flatten()
        .flatten();
        if let Some(entry) = opened {
            let fx = {
                let me: &Game<'_> = self;
                try_with(|st| {
                    let o = st.open.as_mut()?;
                    let id = o.book.clone();
                    let (book, nav) = (st.loaded.get(&id)?, st.navs.get_mut(&id)?);
                    let persist = st.persist.get_or_insert_with(Default::default);
                    let view = o.view.as_ref()?;
                    o.dirty = true;
                    o.search.set_value("");
                    Some(engine::apply_cmd(&me.pt_env(), book, persist, nav, view, &engine::Cmd::Entry(entry), false))
                })
                .flatten()
            };
            if let Some(fx) = fx {
                self.pt_effects(fx);
            }
        }
        true
    }
}

enum Pressed {
    Nothing,
    Fx(Vec<engine::Effect>),
    OpenBook(String),
}

/// The command of the button under the mouse.
fn hit_cmd(view: &engine::View, mx: f64, my: f64) -> Option<engine::Cmd> {
    use engine::Elem;
    let inside = |x: i32, y: i32, w: i32, h: i32| mx >= f64::from(x) && my >= f64::from(y) && mx < f64::from(x + w) && my < f64::from(y + h);
    for e in view.elems.iter().rev() {
        let hit = match e {
            Elem::Button { rect: [x, y, w, h], cmd, .. } if inside(*x, *y, *w, *h) => Some(cmd),
            Elem::EntryBtn { x, y, cmd, .. } if inside(*x, *y, engine::PAGE_WIDTH, 10) => Some(cmd),
            Elem::CatBtn { x, y, cmd, .. } if inside(*x, *y, 20, 20) => Some(cmd),
            Elem::Bookmark { x, y, cmd, .. } if inside(*x, *y, 13, 10) => Some(cmd),
            Elem::MarkRead { x, y, cmd, .. } if inside(*x, *y, 11, 11) => Some(cmd),
            Elem::PlainButton { rect: [x, y, w, h], cmd, .. } if inside(*x, *y, *w, *h) => Some(cmd),
            _ => None,
        };
        if let Some(c) = hit {
            return Some(c.clone());
        }
    }
    None
}

/// The stack under the mouse (for Shift-click).
fn hover_stack(view: &engine::View, open: &Open, mx: f64, my: f64) -> Option<(engine::Stack, usize)> {
    for e in view.elems.iter().rev() {
        if let engine::Elem::Item { stacks, x, y, scale } = e
            && !stacks.is_empty()
            && mx > f64::from(*x)
            && my > f64::from(*y)
            && mx <= f64::from(*x) + f64::from(16.0 * scale)
            && my <= f64::from(*y) + f64::from(16.0 * scale)
        {
            let i = (open.ticks / 20.0) as usize % stacks.len();
            return Some((stacks[i].clone(), i));
        }
    }
    None
}

// ------------------------------------------------------------------------------------------------
// Options > Mods
// ------------------------------------------------------------------------------------------------

impl Game<'_> {
    /// The mod's row in Options > Mods.
    pub(super) fn patchouli_mods(mods: &mut Vec<super::native_hooks::NativeMod>) {
        mods.push(super::native_hooks::NativeMod {
            id: ID.to_owned(),
            title: "Patchouli 1.20.1-85".to_owned(),
            description: "Books for mods: open a guide book to read.".to_owned(),
            keys: Vec::new(),
        });
    }
}
