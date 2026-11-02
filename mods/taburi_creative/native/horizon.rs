// Creative tabs of content mods inside Horizon's creative inventory (creative.rs, the library
// side). The rules are the engine below. `Creative::load` starts reading the content root as it
// loads (a thread, off the loading path); the first time the screen opens (`reopen`) the result
// is added to the `Creative` the game built, and from then on this keeps the page the screen
// shows. The screen's own side (arrows, label, clicks) is in game_ui.rs and reaches this through
// `Creative`'s state getter.
use super::{Creative, ItemStack, SearchEntry, Tab, java_lower, split_id};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
    thread::JoinHandle,
};
#[allow(dead_code)]
mod engine {
    include!("taburi_creative_engine.rs");
}

/// What the content root says about the mods' tabs, read off the loading path.
struct Prepared {
    planned: Vec<engine::PlannedTab>,
    rows: Vec<engine::SearchRow>,
}
/// The reading `Creative::load` started, until the screen first opens.
static PREPARED: Mutex<Option<JoinHandle<Option<Prepared>>>> = Mutex::new(None);
/// The page and which page each tab is on. One screen exists, and `Creative::tab_position`
/// (an associated function, no `self`) has to read it.
static PAGING: Mutex<engine::Paging> = Mutex::new(engine::Paging::empty());

fn paging() -> MutexGuard<'static, engine::Paging> {
    PAGING.lock().unwrap_or_else(|e| e.into_inner())
}

pub const OFF_SCREEN: (f32, f32) = engine::OFF_SCREEN;

/// `Creative::load`'s head: with content mods on in `root`, starts reading their tabs. With none
/// on it does nothing, so a vanilla root gives vanilla's screen, untouched.
pub fn prepare(root: &Path) {
    if crate::mod_content::namespaces(root).is_empty() {
        return;
    }
    let root = root.to_owned();
    let handle = std::thread::spawn(move || read_tabs(&root));
    *PREPARED.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
}

/// The installed content mods' manifests (`horizon-mods/`, as `content_root` finds them).
fn installed_mods() -> Vec<engine::ModInfo> {
    let dir = std::env::current_dir()
        .unwrap_or_default()
        .join(crate::mod_list::DIR);
    let mut out = Vec::new();
    for path in horizon_mods::find(&dir) {
        let manifest = if path.is_dir() {
            fs::read_to_string(path.join(horizon_mods::manifest::MANIFEST))
                .ok()
                .and_then(|text| horizon_mods::Manifest::parse(&text).ok())
        } else {
            horizon_mods::Package::read(&path).ok().map(|p| p.manifest)
        };
        let Some(m) = manifest else {
            continue;
        };
        let mut namespaces = vec![m.id.clone()];
        for ns in &m.namespaces {
            if !namespaces.contains(ns) {
                namespaces.push(ns.clone());
            }
        }
        out.push(engine::ModInfo {
            id: m.id,
            name: m.name,
            namespaces,
        });
    }
    out
}

/// The content namespaces' `itemGroup.*` lang keys (en_us).
fn group_lang(root: &Path, content: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for ns in content {
        let Ok(bytes) = fs::read(root.join(format!("assets/{ns}/lang/en_us.json"))) else {
            continue;
        };
        let Ok(map) = serde_json::from_slice::<BTreeMap<String, serde_json::Value>>(&bytes) else {
            continue;
        };
        for (key, value) in map {
            if engine::is_group_key(&key)
                && let Some(text) = value.as_str()
            {
                out.insert(key, text.to_owned());
            }
        }
    }
    out
}

fn read_tabs(root: &Path) -> Option<Prepared> {
    let content = crate::mod_content::namespaces(root);
    // The registry's order: vanilla's items, then each mod's (items.json of the content root).
    let ids = fs::read(root.join("items.json"))
        .map(|bytes| engine::top_level_keys(&bytes))
        .unwrap_or_default();
    let planned = engine::plan_tabs(
        &ids,
        &content,
        &installed_mods(),
        &group_lang(root, &content),
        engine::ICON_HINTS,
    );
    if planned.is_empty() {
        return None;
    }
    let tags = crate::items::load_tags(root, "item").unwrap_or_default();
    let rows = engine::search_rows(&planned, &tags);
    Some(Prepared { planned, rows })
}

/// Adds the content mods' tabs and items to the `Creative` the game built from vanilla's export,
/// once, the first time the screen opens. Does nothing for a vanilla root.
fn apply(creative: &mut Creative) {
    let Some(handle) = PREPARED.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    // A reading that failed leaves vanilla's screen.
    let Some(Prepared { planned, rows }) = handle.join().ok().flatten() else {
        return;
    };
    let merge = {
        let existing: Vec<engine::TabMeta> = creative
            .tabs
            .iter()
            .map(|t| engine::TabMeta {
                id: &t.id,
                column: t.column,
                search: t.search,
                inventory: t.inventory,
            })
            .collect();
        engine::merge_layout(&existing, &planned)
    };
    let tabs: Vec<Tab> = planned
        .iter()
        .map(|p| Tab {
            id: p.id.clone(),
            title: p.title.clone(),
            icon: p.icon.clone(),
            items: p.items.iter().map(|i| ItemStack::new(i, 1)).collect(),
            top: p.slot.top,
            column: p.slot.column,
            search: false,
            inventory: false,
        })
        .collect();
    // Tabs picked so far keep their place: the mod tabs go after every index in use.
    creative.tabs.splice(merge.insert_at..merge.insert_at, tabs);
    creative.selected = engine::selected_after(creative.selected, merge.insert_at, planned.len());
    // The search tab lists them all, after vanilla's, like every item of a vanilla tab.
    let stacks: Vec<ItemStack> = rows.iter().map(|r| ItemStack::new(&r.item, 1)).collect();
    for tab in creative.tabs.iter_mut().filter(|t| t.search) {
        tab.items.extend(stacks.iter().cloned());
    }
    creative.search_index.extend(rows.iter().map(|r| {
        let (namespace, path) = split_id(&java_lower(&r.item));
        SearchEntry {
            namespace,
            path,
            // Empty: the search then matches the display name from the lang files.
            lines: Vec::new(),
            tags: r.tags.iter().map(|t| split_id(&java_lower(t))).collect(),
        }
    }));
    creative.search_items.extend(stacks);
    // What the grid shows was worked out before: do it again.
    creative.forget_items();
    *paging() = engine::Paging::new(&merge);
}

/// `Creative::tab_position`: whether the tab is on another page than the screen shows.
pub fn off_page(tab: &Tab) -> bool {
    !paging().visible(&tab.id)
}

fn show_selected(creative: &Creative, index: usize) {
    if let Some(tab) = creative.tabs.get(index) {
        paging().show(&tab.id);
    }
}

/// `Creative::reopen`'s head: the mods' tabs are added the first time, then the screen opens on
/// the page of the tab it was left on.
pub fn reopening(creative: &mut Creative) {
    apply(creative);
    show_selected(
        creative,
        creative.selected.min(creative.tabs.len().saturating_sub(1)),
    );
}

/// `Creative::select`'s head: a tab picked some other way than by clicking it (the pad's shoulder
/// buttons) brings its page up.
pub fn selecting(creative: &Creative, index: usize) {
    show_selected(creative, index);
}

/// What the screen's drawing and clicking (game.rs) reads and changes, through the getter
/// `Creative::__horizon_mixins_taburi_creative_paging`.
pub struct PagingHandle;
impl PagingHandle {
    /// Whether there is more than one page (the arrows and the label are shown).
    pub fn paged(&self) -> bool {
        paging().paged()
    }
    pub fn label(&self) -> String {
        paging().label()
    }
    /// The "<" and ">" buttons, `[x, y, width, height]` from the screen's top-left corner.
    pub fn arrows(&self) -> [[f32; 4]; 2] {
        [
            engine::arrow_rect(engine::Arrow::Previous),
            engine::arrow_rect(engine::Arrow::Next),
        ]
    }
    /// Where the label's text starts for its width, from the screen's top-left corner.
    pub fn label_at(&self, width: f32) -> (f32, f32) {
        (engine::label_x(width), engine::LABEL_TOP)
    }
    /// A click at a point relative to the screen's top-left corner; whether an arrow took it.
    pub fn press(&self, x: f32, y: f32) -> bool {
        paging().press(x, y)
    }
    /// Brings up the page of a tab (by id) that is on another one.
    pub fn show(&self, tab_id: &str) -> bool {
        paging().show(tab_id)
    }
}
