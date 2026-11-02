// Explorer's Compass inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the structure
// search (`StructureSearchWorker` and its three kinds, `SearchWorkerManager`), the compass' data on the stack, the HUD lines,
// the teleport and the list screen. This part reads the game: the structure and structure set files, the biome tags, the player,
// the stack in hand, the screen.
//
// The search runs on a thread of its own (a structure start of a village takes 70 to 110 ms to make, so the time slices of
// `WorldWorkerManager` would stop the game); the tick only reads its progress and writes the result on the compass.
//
// The screen: Horizon has no way to register a new screen, so the compass' list takes over the Advancements screen (a plain
// screen over the world that pauses a single-player game, as vanilla's `ExplorersCompassScreen` does): hooks on its draw / mouse /
// drag / scroll, the keys (`sort_key_press`) and the typed text (`recipe_search_focused` + `recipe_key`). Better Advancements
// and Nature's Compass hook the same functions; each answers only while its own list is open.
//
// Server and client (Horizon #1514: single player is an integrated server, the window is its guest), as in the original:
// - the use (`ExplorersCompassItem.use`), the search threads, the compass' data and the teleport run where the world is:
//   the in-frame host's game, or a server, for a guest inside its turn (`as_guest`); each player has his own search;
// - the server tells the guest's client to open the list (`SyncPacket` + `openGUI`) with what only it knows (the
//   structure FOUND, teleport allowed, creative, levels) over foundry's channel `explorerscompass:open`;
// - the list is the client's (it reads the structure files of its own game); its buttons go back to the server on
//   `explorerscompass:ask` (`CompassSearchPacket`, `SearchForNextPacket`, `TeleportPacket`), run in the guest's turn.
#[allow(unused_imports)]
use super::{Dim, Game, Result, Screen, lan::LanHost};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_world::nbt::Tag;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[allow(dead_code)]
mod engine {
    include!("explorerscompass_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "explorerscompass";

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");

/// What the search thread tells the game.
#[derive(Default)]
struct Shared {
    /// `setSearchRadius` calls, in order.
    updates: Vec<i32>,
    done: Option<engine::Outcome>,
    stop: bool,
}
/// The compass' search that runs on its thread.
struct Active {
    shared: Arc<Mutex<Shared>>,
    /// The structure key the compass was told it searches (`structureOrGroupId`).
    key: String,
    /// Where the compass was when the search began; searched again if it moved.
    slot: usize,
}
/// The open screen.
struct Modal {
    model: engine::Model,
    search: horizon_client::ui::EditBox,
    last_cursor: (f32, f32),
}
/// A teleport waiting for its chunk to load: `findValidTeleportHeight` reads blocks.
struct Pending {
    x: i32,
    z: i32,
    since: Instant,
}
/// The structure and structure set files (`worldgen/structure`, `worldgen/structure_set` of every namespace).
#[derive(Default)]
struct Registry {
    /// Structure ids, sorted.
    structures: Vec<String>,
    /// Set id, placement and structures, in the sets' file order.
    sets: Vec<(String, engine::Placement, Vec<String>)>,
    /// Each structure's `biomes`, resolved to biome ids.
    biomes: HashMap<String, HashSet<String>>,
}
/// Whose search or teleport: `None` the in-frame host, `Some(conn)` a guest (`Game::lan_acting` in its turn).
type Who = Option<u32>;
#[derive(Default)]
struct St {
    /// The searches of the players (a server's half).
    active: HashMap<Who, Active>,
    /// The open list (a window's half).
    modal: Option<Modal>,
    /// The teleports waiting for their chunk (a server's half).
    pending: HashMap<Who, Pending>,
    registry: Option<Arc<Registry>>,
}
thread_local! {
    // The integrated server and the window run on different threads: each has its own half.
    static ST: RefCell<St> = RefCell::new(St::default());
}
fn with<R>(f: impl FnOnce(&mut St) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}

/// Server -> client: open the list (`SyncPacket` + `openGUI`): the structure of a FOUND compass, (teleport allowed,
/// creative), the player's levels.
type Open = (Option<String>, (bool, bool), i64);
const OPEN: super::foundry::Channel<Open> = super::foundry::Channel::new("explorerscompass:open");
/// Client -> server: a button of the list: (ASK_SEARCH, (key, is_group)), (ASK_NEXT, ..), (ASK_TELEPORT, ..).
const ASK: super::foundry::Channel<(u8, (String, bool))> = super::foundry::Channel::new("explorerscompass:ask");
const ASK_SEARCH: u8 = 0;
const ASK_NEXT: u8 = 1;
const ASK_TELEPORT: u8 = 2;

fn ec_register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::foundry::register_server_channel(ASK.id, ec_ask_from_client);
}
/// A guest's client pressed a button of the list: it runs as the guest (`as_guest`), as the original's packets run for
/// the `ServerPlayer` who sent them.
fn ec_ask_from_client(game: &mut Game<'_>, host: &mut LanHost, conn: u32, body: &[u8]) {
    let Some((what, (key, is_group))) = ASK.decode(body) else { return };
    game.as_guest(host, conn, |g| g.ec_do(what, &key, is_group));
}
/// Stops a search thread.
fn stop(a: &Active) {
    a.shared.lock().unwrap_or_else(|e| e.into_inner()).stop = true;
}

/// A compass in `stack` whose needle points into another dimension than `dim` (it is written again for this one).
fn stale_target(stack: &ItemStack, dim: &str) -> bool {
    if &*stack.item != engine::ITEM {
        return false;
    }
    let d = read_data(stack);
    let tracker_dim = stack.data().and_then(|c| c.other("minecraft:lodestone_tracker")).and_then(|t| t.get("target")).and_then(|t| horizon_client::save::text(t.get("dimension")));
    d.target().is_some() && tracker_dim.as_deref() != Some(dim)
}

/// `string.explorerscompass.<key>` in the selected language, the English text of the mod's `en_us.json` last.
fn tr(key: &str) -> String {
    let fallback = match key {
        "status" => "Status",
        "searching" => "Searching",
        "found" => "Found",
        "notFound" => "Not Found",
        "structure" => "Structure",
        "radius" => "Radius",
        "coordinates" => "Coordinates",
        "distance" => "Distance",
        "dimension" => "Dimension",
        "source" => "Source",
        "group" => "Group",
        "name" => "Name",
        "levels" => "Levels",
        "search" => "Search",
        "searchForGroup" => "Search for Group",
        "searchForNext" => "Search for Next",
        "selectStructure" => "Select Structure",
        "sortBy" => "Sort By",
        "teleport" => "Teleport",
        other => other,
    };
    horizon_client::lang::get_or(&format!("string.{MOD_ID}.{key}"), fallback)
}
fn lang(key: &str) -> Option<String> {
    let got = horizon_client::lang::get_or(key, key);
    (got != key).then_some(got)
}
/// `StructureUtils.getPrettyStructureName`.
fn pretty(key: &str) -> String {
    let (ns, path) = key.split_once(':').unwrap_or(("minecraft", key));
    engine::pretty_name(key, lang(&format!("structure.{ns}.{}", path.replace('/', "."))))
}

// ------------------------------------------------------------------------------------------------
// The compass' data on the stack (`stack.getTag()` in 1.20.1: `minecraft:custom_data` here)
// ------------------------------------------------------------------------------------------------

fn read_data(stack: &ItemStack) -> engine::Data {
    use horizon_client::save::{number, text};
    let root = stack.data().and_then(|d| d.other("minecraft:custom_data"));
    let get = |k: &str| root.and_then(|t| t.get(k));
    let int = |k: &str| number(get(k)).unwrap_or(0) as i32;
    engine::Data {
        state: int("State"),
        structure_key: get("StructureKey").and_then(|t| text(Some(t))),
        is_group: number(get("IsGroup")).unwrap_or(0) != 0,
        found_x: int("FoundX"),
        found_z: int("FoundZ"),
        prev_pos: match get("PrevPos") {
            Some(Tag::List(list)) => Some(
                list.iter()
                    .map(|c| [number(c.get("X")).unwrap_or(0) as i32, number(c.get("Y")).unwrap_or(0) as i32, number(c.get("Z")).unwrap_or(0) as i32])
                    .collect(),
            ),
            _ => None,
        },
        samples: int("Samples"),
        search_radius: int("SearchRadius"),
        display_coordinates: get("DisplayCoordinates").and_then(|v| number(Some(v))).map(|n| n != 0),
        compass_damage: int("CompassDamage"),
    }
}

/// Writes the data back (the other custom data stays) and keeps the item model's target: `minecraft:lodestone_tracker`
/// (with `tracked` false, so the game's lodestone check leaves it alone) exists only while the compass is FOUND.
fn write_data(stack: &mut ItemStack, d: &engine::Data, dimension: &str) {
    let mut compound = match stack.data().and_then(|x| x.other("minecraft:custom_data")) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Default::default(),
    };
    {
        let mut put = |k: &str, v: Tag| {
            compound.insert(k.into(), v);
        };
        put("State", Tag::Int(d.state));
        if let Some(k) = &d.structure_key {
            put("StructureKey", Tag::String(k.as_str().into()));
        }
        put("IsGroup", Tag::Byte(d.is_group as i8));
        put("FoundX", Tag::Int(d.found_x));
        put("FoundZ", Tag::Int(d.found_z));
        put("Samples", Tag::Int(d.samples));
        put("SearchRadius", Tag::Int(d.search_radius));
        if let Some(b) = d.display_coordinates {
            put("DisplayCoordinates", Tag::Byte(b as i8));
        }
        if d.compass_damage != 0 {
            put("CompassDamage", Tag::Int(d.compass_damage));
        }
    }
    match &d.prev_pos {
        Some(prev) => {
            compound.insert(
                "PrevPos".into(),
                Tag::List(prev.iter().map(|p| Tag::compound([("X", Tag::Int(p[0])), ("Y", Tag::Int(p[1])), ("Z", Tag::Int(p[2]))])).collect()),
            );
        }
        None => {
            compound.remove(&horizon_world::nbt::JavaString::from("PrevPos"));
        }
    }
    let tracker = d.target().map(|(x, z)| {
        Tag::compound([
            ("target", Tag::compound([("dimension", Tag::String(dimension.into())), ("pos", Tag::IntArray(vec![x, 0, z]))])),
            ("tracked", Tag::Byte(0)),
        ])
    });
    let data = stack.data_mut();
    data.set_other("minecraft:custom_data", Some(Tag::Compound(compound)));
    data.set_other("minecraft:lodestone_tracker", tracker);
}

fn dimension_id(dim: Dim) -> &'static str {
    match dim {
        Dim::Nether => "minecraft:the_nether",
        Dim::End => "minecraft:the_end",
        _ => "minecraft:overworld",
    }
}

/// The world the search reads (its own thread: the terrain is shared, its queries are locked inside).
struct TerrainWorld {
    terrain: Arc<horizon_client::vanillagen::VanillaTerrain>,
    seed: i64,
}
impl engine::StructureWorld for TerrainWorld {
    fn starts(&mut self, chunk_x: i32, chunk_z: i32) -> Vec<engine::Start> {
        self.terrain
            .starts_at(chunk_x, chunk_z)
            .iter()
            .map(|s| engine::Start { structure: s.name.clone(), chunk: [s.chunk.x, s.chunk.z] })
            .collect()
    }
    fn rings(&mut self, set: &str) -> Vec<[i32; 2]> {
        self.terrain.ring_positions(set).unwrap_or_default().iter().map(|c| [c.x, c.z]).collect()
    }
    fn seed(&self) -> i64 {
        self.seed
    }
}

/// A teleport column as `isFree` sees it.
struct Live<'a, 'g> {
    game: &'a Game<'g>,
    x: i32,
    z: i32,
}
impl engine::Column for Live<'_, '_> {
    fn free(&self, y: i32) -> bool {
        let g = self.game;
        let id = g.world.block(self.x, y, self.z);
        let facts = g.res.facts().get(id);
        g.res.facts().is_air(id)
            || kit_blocks::block_in_tag(g, "minecraft:fire", &facts.name)
            || matches!(&*facts.name, "minecraft:water" | "minecraft:lava" | "minecraft:bubble_column")
            || facts.replaceable
    }
    fn outside(&self, y: i32) -> bool {
        y < self.game.dim.min_y() || y > self.game.dim.max_y()
    }
}

/// The ids in a `biomes` field: a tag (`#ns:tag`), an id or a list of both.
fn biome_ids(game: &Game<'_>, v: &serde_json::Value, out: &mut HashSet<String>) {
    match v {
        serde_json::Value::String(s) => match s.strip_prefix('#') {
            Some(tag) => {
                if let Some(set) = game.res.mob_data().biome_tags.get(tag) {
                    out.extend(set.iter().cloned());
                }
            }
            None => {
                out.insert(s.clone());
            }
        },
        serde_json::Value::Array(a) => a.iter().for_each(|e| biome_ids(game, e, out)),
        _ => {}
    }
}

impl Game<'_> {
    /// `ItemUtils.getHeldItem`: the slot of the compass in the main hand, else in the off hand.
    fn ec_slot(&self) -> Option<usize> {
        let is = |slot: usize| self.stack_in(slot).is_some_and(|s| &*s.item == engine::ITEM);
        if is(self.selected) {
            Some(self.selected)
        } else if is(super::shield::OFFHAND_SLOT) {
            Some(super::shield::OFFHAND_SLOT)
        } else {
            None
        }
    }

    // ---- use ----------------------------------------------------------------------------------

    /// HEAD on `throw_item`: `ExplorersCompassItem.use` (the original answers PASS).
    ///
    /// A server action: the in-frame host's game opens its own list; a server, in the guest's turn, stops the guest's
    /// search (sneaking) or tells the guest's client to open the list. A server's client only sends the use.
    pub(super) fn explorerscompass_use(&mut self, stack: &ItemStack) -> bool {
        if &*stack.item != engine::ITEM || self.remote.is_some() || self.screen != Screen::Playing {
            return false;
        }
        ec_register();
        let Some(slot) = self.ec_slot() else { return false };
        let who = self.lan_acting;
        if kit_blocks::sneaking(self) || (who.is_some() && self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)) {
            // Crouching: the workers stop and the places found are forgotten, the compass is inactive.
            with(|st| {
                if let Some(a) = st.active.remove(&who) {
                    stop(&a);
                }
            });
            self.ec_update(slot, |d| d.deactivate());
            return true;
        }
        let data = read_data(stack);
        if data.is_broken(engine::config::GENERAL_COMPASS_DURABILITY) {
            return true;
        }
        let found = (data.state() == Some(engine::State::Found)).then(|| data.structure_key.clone()).flatten();
        let can_teleport = engine::config::GENERAL_ALLOW_TELEPORT && self.allow_commands();
        let (creative, levels) = (self.creative, i64::from(self.experience.level));
        match who {
            None => self.ec_show(found, can_teleport, creative, levels),
            // A client without foundry (vanilla) has no list to show.
            Some(conn) => {
                if super::foundry::guest_has_foundry(conn) {
                    super::foundry::send_to_acting(self, OPEN.id, &OPEN.encode(&(found, (can_teleport, creative), levels)));
                }
            }
        }
        true
    }

    /// A button of the list, where the world is (the in-frame host, or a server in the guest's turn).
    fn ec_do(&mut self, what: u8, key: &str, is_group: bool) {
        match what {
            ASK_SEARCH => self.ec_search_for(key, is_group),
            ASK_NEXT => self.ec_search_next(),
            ASK_TELEPORT => self.ec_teleport(),
            _ => {}
        }
    }

    /// The list's button pressed in this window: run here (in-frame host) or sent to the server.
    fn ec_ask(&mut self, what: u8, key: String, is_group: bool) {
        if self.remote.is_some() {
            super::foundry::send_to_server(self, ASK.id, &ASK.encode(&(what, (key, is_group))));
        } else {
            self.ec_do(what, &key, is_group);
        }
    }

    fn ec_update(&mut self, slot: usize, f: impl FnOnce(&mut engine::Data)) -> bool {
        let dim = dimension_id(self.dim);
        let Some(stack) = self.stack_in_mut(slot).as_mut().filter(|s| &*s.item == engine::ITEM) else {
            return false;
        };
        let mut d = read_data(stack);
        f(&mut d);
        write_data(stack, &d, dim);
        true
    }
    fn ec_data(&self, slot: usize) -> Option<engine::Data> {
        self.stack_in(slot).filter(|s| &*s.item == engine::ITEM).map(read_data)
    }

    // ---- the registries -----------------------------------------------------------------------

    /// The structure and structure set files of every namespace of the game's data.
    fn ec_registry(&self) -> Arc<Registry> {
        if let Some(r) = with(|st| st.registry.clone()) {
            return r;
        }
        let mut reg = Registry::default();
        let data = self.options.resources.join("data");
        let mut namespaces: Vec<_> = std::fs::read_dir(&data).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        namespaces.sort();
        fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<(String, serde_json::Value)>) {
            let mut entries: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
            entries.sort();
            for p in entries {
                if p.is_dir() {
                    walk(&p, base, out);
                } else if p.extension().is_some_and(|e| e == "json")
                    && let Ok(bytes) = std::fs::read(&p)
                    && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
                {
                    let rel = p.strip_prefix(base).map(|r| r.with_extension("").to_string_lossy().replace('\\', "/")).unwrap_or_default();
                    out.push((rel, v));
                }
            }
        }
        for ns_dir in namespaces {
            let Some(ns) = ns_dir.file_name().and_then(|n| n.to_str()).map(str::to_owned) else { continue };
            let mut files = Vec::new();
            let dir = ns_dir.join("worldgen/structure");
            walk(&dir, &dir, &mut files);
            for (name, v) in files {
                let id = format!("{ns}:{name}");
                let mut biomes = HashSet::new();
                biome_ids(self, &v["biomes"], &mut biomes);
                reg.biomes.insert(id.clone(), biomes);
                reg.structures.push(id);
            }
            let mut sets = Vec::new();
            let dir = ns_dir.join("worldgen/structure_set");
            walk(&dir, &dir, &mut sets);
            for (name, v) in sets {
                let placement = &v["placement"];
                let kind = placement["type"].as_str().unwrap_or("");
                let int = |k: &str| placement[k].as_i64().unwrap_or(0) as i32;
                let placement = match kind {
                    "minecraft:random_spread" => engine::Placement::RandomSpread {
                        spacing: int("spacing").max(1),
                        separation: int("separation"),
                        salt: int("salt"),
                        triangular: placement["spread_type"].as_str() == Some("triangular"),
                    },
                    "minecraft:concentric_rings" => engine::Placement::Rings,
                    _ => engine::Placement::Generic,
                };
                let structures: Vec<String> = v["structures"].as_array().into_iter().flatten().filter_map(|e| e["structure"].as_str().map(str::to_owned)).collect();
                reg.sets.push((format!("{ns}:{name}"), placement, structures));
            }
        }
        reg.structures.sort();
        let reg = Arc::new(reg);
        with(|st| st.registry = Some(reg.clone()));
        reg
    }

    /// Dimension names for the biomes (`is_overworld`, `is_nether`, `is_end`: what each dimension's biome source lists).
    fn ec_dimension_biomes(&self, dim: &str) -> HashSet<String> {
        self.res.mob_data().biome_tags.get(&format!("minecraft:is_{dim}")).cloned().unwrap_or_default()
    }

    /// `StructureUtils.getGeneratingDimensionKeys` as display names.
    fn ec_dimensions(&self, reg: &Registry, key: &str) -> String {
        let Some(biomes) = reg.biomes.get(key) else { return String::new() };
        let mut names = Vec::new();
        for (dim, id, english) in [("overworld", "dimension.minecraft.overworld", "Overworld"), ("nether", "dimension.minecraft.the_nether", "The Nether"), ("end", "dimension.minecraft.the_end", "The End")] {
            if self.ec_dimension_biomes(dim).iter().any(|b| biomes.contains(b)) {
                names.push(lang(id).unwrap_or_else(|| english.to_owned()));
            }
        }
        names.join(", ")
    }

    /// `StructureUtils.getTypeForStructure`: the first set that holds the structure.
    fn ec_group(reg: &Registry, key: &str) -> String {
        reg.sets.iter().find(|(_, _, s)| s.iter().any(|x| x == key)).map_or_else(|| engine::NO_GROUP.to_owned(), |(id, _, _)| id.clone())
    }

    /// The sets of a structure that can generate in the current dimension (`getPlacementsForStructure`).
    fn ec_sets_of(&self, reg: &Registry, structure: &str) -> Vec<engine::SetDef> {
        let dim = match self.dim {
            Dim::Nether => "nether",
            Dim::End => "end",
            _ => "overworld",
        };
        let here = self.ec_dimension_biomes(dim);
        reg.sets
            .iter()
            .filter(|(_, _, members)| members.iter().any(|m| m == structure))
            // The set is possible when one of its structures has a biome of this dimension.
            .filter(|(_, _, members)| members.iter().any(|m| reg.biomes.get(m).is_some_and(|b| b.iter().any(|x| here.contains(x)))))
            .map(|(id, placement, _)| engine::SetDef { id: id.clone(), placement: placement.clone(), structures: vec![] })
            .collect()
    }

    /// The allowed structures (`getAllowedStructureKeys`) as list rows.
    fn ec_entries(&self) -> Vec<engine::Entry> {
        let reg = self.ec_registry();
        let mut out = Vec::new();
        for key in &reg.structures {
            if engine::blacklisted(engine::config::GENERAL_STRUCTURE_BLACKLIST, key) {
                continue;
            }
            let ns = key.split_once(':').map_or("minecraft", |(n, _)| n);
            let group_key = Self::ec_group(&reg, key);
            out.push(engine::Entry {
                key: key.clone(),
                name: pretty(key),
                source: if ns == "minecraft" { "Minecraft".to_owned() } else { ns.to_owned() },
                group: pretty(&group_key),
                group_key,
                dimensions: self.ec_dimensions(&reg, key),
                xp: engine::xp_levels(key),
            });
        }
        out
    }

    // ---- the list ----------------------------------------------------------------------------

    /// Opens the list over the world in this window with what the server said: the structure of a FOUND compass, whether
    /// teleporting is allowed, creative (`infiniteXp`), the player's levels.
    fn ec_show(&mut self, found: Option<String>, can_teleport: bool, creative: bool, levels: i64) {
        let model = engine::Model::new(self.ec_entries(), found, can_teleport, engine::config::GENERAL_MAX_NEXT_SEARCHES, creative, levels);
        let mut search = horizon_client::ui::EditBox::new(0.0, 0.0, 0.0, 8.0);
        search.bordered = false;
        search.max_length = 32;
        search.text_color = u32::MAX;
        with(|st| st.modal = Some(Modal { model, search, last_cursor: self.cursor }));
        self.screen = Screen::Advancements;
        self.capture(false);
    }

    /// HEAD on `native_tick` (every `Game`, the window included): the list goes when its screen is gone; a server's client
    /// opens the list its server asks for.
    pub(super) fn explorerscompass_client_tick(&mut self) {
        if self.screen != Screen::Advancements && with(|st| st.modal.is_some()) {
            with(|st| st.modal = None);
        }
        if self.remote.is_none() {
            return;
        }
        for body in super::foundry::client_take(OPEN.id) {
            let Some((found, (can_teleport, creative), levels)) = OPEN.decode(&body) else { continue };
            if self.screen == Screen::Playing {
                self.ec_show(found, can_teleport, creative, levels);
            }
        }
    }

    fn ec_close(&mut self) {
        with(|st| st.modal = None);
        if self.screen == Screen::Advancements {
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }

    fn ec_modal_open(&self) -> bool {
        self.screen == Screen::Advancements && ST.with(|s| s.try_borrow().map(|s| s.modal.is_some()).unwrap_or(true))
    }

    // ---- searching ---------------------------------------------------------------------------

    /// `searchForStructure(level, player, pos, key, isGroup, stack)`.
    fn ec_search_for(&mut self, key: &str, is_group: bool) {
        let Some(slot) = self.ec_slot() else { return };
        if self.ec_data(slot).is_none_or(|d| d.is_broken(engine::config::GENERAL_COMPASS_DURABILITY)) {
            return;
        }
        let reg = self.ec_registry();
        let structures: Vec<String> = if is_group {
            reg.sets.iter().find(|(id, _, _)| id == key).map(|(_, _, s)| s.clone()).unwrap_or_default()
        } else {
            vec![key.to_owned()]
        };
        self.ec_update(slot, |d| {
            d.set_searching(key);
            d.search_radius = 0;
            d.is_group = is_group;
        });
        self.ec_launch(slot, key, key, &structures, Vec::new());
        // `player.giveExperienceLevels(-xpLevelCost)` unless creative.
        let levels = engine::xp_levels(key);
        if !self.creative && levels > 0 {
            self.give_levels(-(levels as i32));
        }
    }

    /// `searchForNextStructure`.
    fn ec_search_next(&mut self) {
        let Some(slot) = self.ec_slot() else { return };
        let Some(d) = self.ec_data(slot) else { return };
        if d.is_broken(engine::config::GENERAL_COMPASS_DURABILITY) {
            return;
        }
        let Some(structure_key) = d.structure_key.clone() else { return };
        let prev = d.prev_pos.clone().unwrap_or_default();
        let reg = self.ec_registry();
        let (searching, structures) = if d.is_group {
            // The compass keeps the structure found: the group is worked out again.
            let group = Self::ec_group(&reg, &structure_key);
            let list = reg.sets.iter().find(|(id, _, _)| *id == group).map(|(_, _, s)| s.clone()).unwrap_or_default();
            (group, list)
        } else {
            (structure_key.clone(), vec![structure_key.clone()])
        };
        self.ec_update(slot, |d| {
            d.search_radius = 0;
            d.set_searching(&searching);
        });
        self.ec_launch(slot, &structure_key, &structure_key, &structures, prev);
        let levels = engine::xp_levels(&structure_key);
        if !self.creative && levels > 0 {
            self.give_levels(-(levels as i32));
        }
    }

    /// Makes the workers and starts the thread. `fail_key` is what a failed search writes as `StructureKey`.
    fn ec_launch(&mut self, slot: usize, key: &str, fail_key: &str, structures: &[String], prev: Vec<[i32; 3]>) {
        let Some(p) = self.player.as_ref().map(|p| p.position) else { return };
        let start = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        let reg = self.ec_registry();
        let sets_of = |s: &str| self.ec_sets_of(&reg, s);
        let mut search = engine::Search::new(fail_key, structures, &sets_of, start, prev);
        let terrain = self.end.locator();
        let who = self.lan_acting;
        // Whatever this player's compass searched before stops (each player has his own search).
        with(|st| {
            if let Some(a) = st.active.remove(&who) {
                stop(&a);
            }
        });
        let Some(terrain) = terrain.filter(|_| !search.is_empty()) else {
            // No worker (the structure does not generate in this dimension), or no generated terrain: not found.
            self.ec_apply(slot, key, engine::Outcome::NotFound { radius: 0, samples: 0 });
            return;
        };
        let shared = Arc::new(Mutex::new(Shared::default()));
        let thread_shared = shared.clone();
        let seed = self.level.seed;
        let spawned = std::thread::Builder::new().name("explorerscompass-search".into()).spawn(move || {
            let mut world = TerrainWorld { terrain, seed };
            loop {
                if thread_shared.lock().unwrap_or_else(|e| e.into_inner()).stop {
                    break;
                }
                let outcome = search.do_work(&mut world);
                let mut s = thread_shared.lock().unwrap_or_else(|e| e.into_inner());
                s.updates.append(&mut search.radius_updates);
                if let Some(o) = outcome {
                    s.done = Some(o);
                    break;
                }
            }
        });
        match spawned {
            Ok(_) => with(|st| {
                st.active.insert(who, Active { shared, key: key.to_owned(), slot });
            }),
            Err(_) => self.ec_apply(slot, key, engine::Outcome::NotFound { radius: 0, samples: 0 }),
        }
    }

    /// `ExplorersCompassItem.succeed` / `fail`.
    fn ec_apply(&mut self, slot: usize, key: &str, o: engine::Outcome) {
        // The compass may have moved in the inventory meanwhile: the SEARCHING one.
        let slot = if self.ec_data(slot).is_some() {
            slot
        } else {
            let mut slots: Vec<usize> = (0..self.inventory.len()).collect();
            slots.push(super::shield::OFFHAND_SLOT);
            match slots.into_iter().find(|&s| self.ec_data(s).is_some_and(|d| d.state() == Some(engine::State::Searching))) {
                Some(s) => s,
                None => return,
            }
        };
        let durability = engine::config::GENERAL_COMPASS_DURABILITY;
        match o {
            engine::Outcome::Found { structure, x, z, prev_pos, samples } => {
                let is_group = self.ec_data(slot).is_some_and(|d| d.is_group);
                self.ec_update(slot, |d| d.succeed(&structure, is_group, x, z, prev_pos, samples, engine::config::GENERAL_DISPLAY_COORDINATES, durability));
            }
            engine::Outcome::NotFound { radius, samples } => {
                // `fail` writes the structure key the workers were given (`structureOrGroupId`).
                self.ec_update(slot, |d| d.fail(key, radius, samples));
            }
        }
    }

    /// HEAD on `tick_blocks`: the in-frame host's search, teleport and compasses. A server has no player of its own
    /// (`self.player` is `None` on the integrated server): its guests tick in `tick_guests`.
    pub(super) fn explorerscompass_tick(&mut self) {
        if self.remote.is_some() || self.lan_acting.is_some() || self.player.is_none() {
            return;
        }
        ec_register();
        self.ec_player_tick();
    }

    /// HEAD on `tick_guests` (the guests' tick on the server; single player's window is a guest): each guest with a search,
    /// a teleport waiting or a compass to point again ticks as the guest.
    pub(super) fn explorerscompass_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        ec_register();
        let all: Vec<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        // Who left: his search thread stops.
        with(|st| {
            st.active.retain(|w, a| {
                let keep = w.is_none_or(|c| all.contains(&c));
                if !keep {
                    stop(a);
                }
                keep
            });
            st.pending.retain(|w, _| w.is_none_or(|c| all.contains(&c)));
        });
        let dim = dimension_id(self.dim);
        let conns: Vec<u32> = host
            .hub
            .guests
            .iter()
            .filter(|g| host.hub.here(g) && g.loaded && g.placed && !g.dead)
            .filter(|g| {
                let busy = with(|st| st.active.contains_key(&Some(g.conn)) || st.pending.contains_key(&Some(g.conn)));
                busy || g.inventory.iter().chain(std::iter::once(&g.offhand)).flatten().any(|s| stale_target(s, dim))
            })
            .map(|g| g.conn)
            .collect();
        for conn in conns {
            self.as_guest(host, conn, |g| g.ec_player_tick());
        }
    }

    /// The tick of the player in place (the in-frame host, or a guest in its turn): the search thread's progress goes on the
    /// compass; a teleport waiting for its chunk goes on; the compasses keep their target in this dimension.
    fn ec_player_tick(&mut self) {
        let who = self.lan_acting;
        self.ec_tick_teleport();
        let dim = dimension_id(self.dim);
        let mut slots: Vec<usize> = (0..self.inventory.len()).collect();
        slots.push(super::shield::OFFHAND_SLOT);
        for s in slots {
            if self.stack_in(s).is_some_and(|x| stale_target(x, dim)) {
                self.ec_update(s, |_| {});
            }
        }
        let Some((shared, slot, key)) = with(|st| st.active.get(&who).map(|a| (a.shared.clone(), a.slot, a.key.clone()))) else { return };
        let (updates, done) = {
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            (std::mem::take(&mut s.updates), s.done.take())
        };
        for r in updates {
            // `setSearchRadius`, every 250 blocks.
            if self.ec_data(slot).is_some() {
                self.ec_update(slot, |d| d.search_radius = r);
            }
        }
        if let Some(o) = done {
            with(|st| st.active.remove(&who));
            self.ec_apply(slot, &key, o);
        }
    }

    // ---- teleport ----------------------------------------------------------------------------

    fn ec_teleport(&mut self) {
        let Some(slot) = self.ec_slot() else { return };
        if !(engine::config::GENERAL_ALLOW_TELEPORT && self.allow_commands()) {
            return;
        }
        let Some((x, z)) = self.ec_data(slot).and_then(|d| d.target()) else { return };
        if self.world.loaded(x, z) {
            self.ec_teleport_to(x, z);
        } else {
            let who = self.lan_acting;
            with(|st| st.pending.insert(who, Pending { x, z, since: Instant::now() }));
        }
    }
    /// The player in place goes to the column (a guest's client is teleported by `leave_guest`).
    fn ec_teleport_to(&mut self, x: i32, z: i32) {
        let y = engine::teleport_height(&Live { game: self, x, z }, 63);
        self.stop_riding();
        let host = self.lan_acting.is_none();
        if let Some(p) = self.player.as_mut() {
            let v = horizon_core::geometry::Vec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5);
            p.position = v;
            p.velocity = horizon_core::geometry::Vec3::new(p.velocity.x, 0.0, p.velocity.z);
            p.fall_distance = 0.0;
            p.ground.on_ground = true;
            // The window's interpolation start is the in-frame host's own.
            if host {
                self.previous_position = v;
            }
        }
    }
    fn ec_tick_teleport(&mut self) {
        let who = self.lan_acting;
        let Some((x, z, since)) = with(|st| st.pending.get(&who).map(|p| (p.x, p.z, p.since))) else { return };
        if self.world.loaded(x, z) {
            with(|st| st.pending.remove(&who));
            self.ec_teleport_to(x, z);
        } else if since.elapsed() > Duration::from_secs(20) {
            with(|st| st.pending.remove(&who));
        }
    }

    // ---- HUD ---------------------------------------------------------------------------------

    /// HEAD on `draw_selected_name`: `ExplorersCompassOverlay.render`.
    pub(super) fn explorerscompass_hud(&self, batch: &mut HudBatch) {
        let show_screen = self.screen == Screen::Playing || (self.screen == Screen::Chat && engine::config::CLIENT_DISPLAY_WITH_CHAT_OPEN);
        if !show_screen || self.debug || self.ui.hide_gui {
            return;
        }
        let Some(slot) = self.ec_slot() else { return };
        let Some(p) = self.player.as_ref().map(|p| p.position) else { return };
        let Some(data) = self.ec_data(slot) else { return };
        let name = data.structure_key.as_deref().map(pretty).unwrap_or_default();
        let player = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        let atlas = &self.gui.atlas;
        for l in engine::overlay_lines(&data, player, &name, &tr) {
            let w = HudBatch::text_width(atlas, &l.text);
            let (x, y, shadow) = engine::hud_position(l.rel, batch.width, w);
            let c = rgba((l.color >> 16) as u8, (l.color >> 8) as u8, l.color as u8, 255);
            batch.text(atlas, &l.text, x, y, c, shadow);
        }
    }

    // ---- the screen --------------------------------------------------------------------------

    pub(super) fn explorerscompass_draw(&mut self, batch: &mut HudBatch) -> bool {
        if !self.ec_modal_open() {
            return false;
        }
        let me: &Game<'_> = self;
        ST.with(|s| {
            if let Ok(mut st) = s.try_borrow_mut()
                && let Some(m) = st.modal.as_mut()
            {
                me.ec_paint(batch, m);
            }
        });
        true
    }

    fn ec_button(&self, batch: &mut HudBatch, r: engine::Rect, label: &str, active: bool) {
        let atlas = &self.gui.atlas;
        let hovered = r.contains(self.cursor.0, self.cursor.1);
        let a = engine::button_alpha(active, hovered);
        batch.fill(atlas, r.x as f32, r.y as f32, r.w as f32, r.h as f32, rgba(0, 0, 0, a));
        let w = HudBatch::text_width(atlas, label);
        batch.text(atlas, label, (r.x as f32 + (r.w as f32 - w) / 2.0).floor(), (r.y + (r.h - 8) / 2) as f32, u32::MAX, true);
    }

    fn ec_paint(&self, batch: &mut HudBatch, m: &mut Modal) {
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width as i32, batch.height as i32);
        // `renderBackground`: the world darkened (-1072689136 -> -804253680).
        batch.fill_gradient(atlas, (0.0, 0.0, w as f32, h as f32), rgba(0x10, 0x10, 0x10, 0xC0), rgba(0x10, 0x10, 0x10, 0xD0));
        let l = m.model.layout(w, h);
        let title = tr("selectStructure");
        let tw = HudBatch::text_width(atlas, &title);
        batch.text(atlas, &title, (65.0 - tw / 2.0).floor(), 15.0, u32::MAX, true);
        let active = m.model.search_active();
        self.ec_button(batch, l.search, &tr("search"), active);
        self.ec_button(batch, l.group, &tr("searchForGroup"), active);
        if let Some(n) = l.next {
            self.ec_button(batch, n, &tr("searchForNext"), m.model.found_selected());
        }
        let label = format!("{}: {}", tr("sortBy"), tr(m.model.sorting.label().0));
        self.ec_button(batch, l.sort, &label, true);
        if let Some(t) = l.teleport {
            self.ec_button(batch, t, &tr("teleport"), m.model.found_selected());
        }
        self.ec_button(batch, l.cancel, &horizon_client::lang::get_or("gui.cancel", "Cancel"), true);
        // The search field (`TransparentTextField`).
        let s = l.field;
        batch.fill(atlas, s.x as f32, s.y as f32, s.w as f32, s.h as f32, rgba(0, 0, 0, 127));
        m.search.x = s.x as f32 + 4.0;
        m.search.y = s.y as f32 + ((s.h - 8) / 2) as f32;
        m.search.width = s.w as f32 - 8.0;
        m.search.height = 8.0;
        if m.search.value.is_empty() && !m.search.focused {
            batch.text(atlas, &tr("search"), m.search.x, m.search.y, rgba(0x80, 0x80, 0x80, 255), true);
        } else {
            m.search.draw(batch, atlas, None);
        }
        let list = l.list;
        if m.model.scroll_drag {
            m.model.drag_scroll(self.cursor.1 - m.last_cursor.1, list);
        }
        m.last_cursor = self.cursor;
        let start = batch.vertices.len();
        for (i, &e) in m.model.rows.iter().enumerate() {
            let top = m.model.row_top(i, list);
            if top + engine::ROW_HEIGHT < list.y || top > list.y + list.h {
                continue;
            }
            let entry = &m.model.entries[e];
            let enabled = m.model.enabled(entry);
            let selected = m.model.selected.as_deref() == Some(entry.key.as_str());
            let left = list.x + 2;
            batch.fill(atlas, left as f32, top as f32, (list.x + list.w - left) as f32, engine::ROW_HEIGHT as f32, rgba(0, 0, 0, engine::row_alpha(enabled, selected)));
            self.ec_row(batch, entry, left, top, list.w, enabled);
        }
        batch.clip_since(start, list.x as f32, list.y as f32, (list.x + list.w) as f32, (list.y + list.h) as f32);
        if let Some((top, th)) = m.model.thumb(list) {
            let x = (list.x + list.w) as f32;
            batch.fill(atlas, x, list.y as f32, engine::SCROLLBAR_WIDTH as f32, list.h as f32, rgba(0, 0, 0, engine::row_alpha(false, false)));
            batch.fill(atlas, x, top as f32, engine::SCROLLBAR_WIDTH as f32, th as f32, rgba(0, 0, 0, engine::row_alpha(true, true)));
        }
    }

    /// `StructureSearchEntry.render`.
    fn ec_row(&self, batch: &mut HudBatch, e: &engine::Entry, left: i32, top: i32, width: i32, enabled: bool) {
        let atlas = &self.gui.atlas;
        let xp = e.xp.min(3);
        if xp > 0 {
            // The orb sprites are textures of the mod, which a native mod cannot draw: the level count in green.
            let c = if enabled { rgba(0x80, 0xFF, 0x20, 255) } else { rgba(0x40, 0x7F, 0x10, 255) };
            let t = format!("{xp}");
            let tw = HudBatch::text_width(atlas, &t);
            batch.text(atlas, &t, (left + width - 20) as f32 - tw / 2.0 + 8.0, (top + 21) as f32, c, true);
        }
        let name_color = if enabled { u32::MAX } else { rgba(0x80, 0x80, 0x80, 255) };
        let info_color = if enabled { rgba(0x80, 0x80, 0x80, 255) } else { rgba(0x55, 0x55, 0x55, 255) };
        let line = 11; // font.lineHeight + 2
        let mid = top + engine::ROW_HEIGHT / 2;
        batch.text(atlas, &e.name, (left + 5) as f32, (mid - line * 2) as f32, name_color, true);
        batch.text(atlas, &format!("{}: {}", tr("source"), e.source), (left + 5) as f32, (mid - line) as f32, info_color, true);
        batch.text(atlas, &format!("{}: {}", tr("group"), e.group), (left + 5) as f32, mid as f32, info_color, true);
        batch.text(atlas, &format!("{}: {}", tr("dimension"), e.dimensions), (left + 5) as f32, (mid + line) as f32, info_color, true);
    }

    pub(super) fn explorerscompass_mouse(&mut self, pressed: bool, size: [u32; 2]) -> bool {
        if !self.ec_modal_open() {
            return false;
        }
        if !pressed {
            with(|st| {
                if let Some(m) = st.modal.as_mut() {
                    m.model.scroll_drag = false;
                }
            });
            return true;
        }
        let batch = HudBatch::new(size);
        let (w, h) = (batch.width as i32, batch.height as i32);
        let now = self.started.elapsed().as_millis() as u64;
        let (px, py) = self.cursor;
        let action = with(|st| {
            let m = st.modal.as_mut()?;
            m.search.focused = false;
            Some(m.model.click(px, py, w, h, now))
        });
        let Some(action) = action else { return true };
        self.audio.play_button_click();
        match action {
            engine::Action::None => {}
            engine::Action::FocusSearch => {
                with(|st| {
                    if let Some(m) = st.modal.as_mut() {
                        m.search.focused = true;
                        m.search.reset_blink();
                        m.search.click(&self.gui.atlas, px, self.has_shift_down(), false);
                    }
                });
            }
            engine::Action::Close => self.ec_close(),
            // `CompassSearchPacket`, `SearchForNextPacket`, `TeleportPacket`: to the server (or here, in frame).
            engine::Action::Search(key) => {
                self.ec_close();
                self.ec_ask(ASK_SEARCH, key, false);
            }
            engine::Action::SearchGroup(group) => {
                self.ec_close();
                self.ec_ask(ASK_SEARCH, group, true);
            }
            engine::Action::SearchNext => {
                self.ec_close();
                self.ec_ask(ASK_NEXT, String::new(), false);
            }
            engine::Action::Teleport => {
                self.ec_close();
                self.ec_ask(ASK_TELEPORT, String::new(), false);
            }
        }
        true
    }

    pub(super) fn explorerscompass_drag(&mut self) -> bool {
        self.ec_modal_open()
    }

    pub(super) fn explorerscompass_scroll(&mut self, delta: f32) -> bool {
        if !self.ec_modal_open() {
            return false;
        }
        let size = self.renderer.as_ref().map_or([1, 1], |r| [r.config.width, r.config.height]);
        let b = HudBatch::new(size);
        let (w, h) = (b.width as i32, b.height as i32);
        with(|st| {
            if let Some(m) = st.modal.as_mut() {
                let list = m.model.layout(w, h).list;
                m.model.wheel(delta.signum(), list);
            }
        });
        true
    }

    pub(super) fn explorerscompass_key(&mut self, key: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode as K;
        if !self.ec_modal_open() {
            return false;
        }
        if key == K::Escape {
            self.ec_close();
            return true;
        }
        !matches!(key, K::F1 | K::F2 | K::F11)
    }

    pub(super) fn explorerscompass_typing(&self) -> bool {
        self.ec_modal_open() && ST.with(|s| s.try_borrow().map(|s| s.modal.as_ref().is_some_and(|m| m.search.focused)).unwrap_or(false))
    }

    pub(super) fn explorerscompass_type(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode as K, PhysicalKey};
        if !self.explorerscompass_typing() {
            return false;
        }
        let shortcut = self.edit_shortcut(event);
        let (ctrl, shift) = (self.has_control_down(), self.has_shift_down());
        let clipboard = &mut self.clipboard;
        with(|st| {
            let Some(m) = st.modal.as_mut() else { return };
            if let Some(sc) = shortcut {
                m.search.shortcut(sc, clipboard);
            } else {
                match event.physical_key {
                    PhysicalKey::Code(K::Enter | K::NumpadEnter) => {}
                    PhysicalKey::Code(code) if m.search.key(code, ctrl, shift) => {}
                    _ => {
                        if !ctrl && let Some(text) = &event.text {
                            let text: String = text.chars().filter(|c| !c.is_control()).collect();
                            m.search.insert_text(&text);
                        }
                    }
                }
            }
            m.search.reset_blink();
            let value = m.search.value.clone();
            m.model.set_term(&value);
        });
        true
    }

    /// HEAD on `load_world_state`: a world opens; the thread of the one before stops, nothing of it stays.
    pub(super) fn explorerscompass_world_opened() {
        with(|st| {
            for a in st.active.values() {
                stop(a);
            }
            *st = St::default();
        });
    }
}

#[allow(dead_code)]
fn _keep(_: BTreeSet<u8>) {}
