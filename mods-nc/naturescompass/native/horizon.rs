// Nature's Compass inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the biome
// search (`BiomeSearchWorker`), the compass' data on the stack, the HUD lines, the teleport and the list screen.
// This part reads the game: the biomes and their tags, the player, the stack in hand, the screen.
//
// The screen: Horizon has no way to register a new screen, so the compass' list takes over the Advancements screen
// (a plain screen over the world that pauses a single-player game, as vanilla's `NaturesCompassScreen` does, since
// `Screen.isPauseScreen` is true by default): hooks on its draw / mouse / drag / scroll, the keys (`sort_key_press`)
// and the typed text (`recipe_search_focused` + `recipe_key`). Better Advancements hooks the same functions with a
// lower priority and is never reached while this screen is open.
//
// Server and client (Horizon #1514: single player is an integrated server, the window is its guest), as in the original:
// - the use (`NaturesCompassItem.use`), the search (`BiomeSearchWorker`), the compass' data and the teleport run where the
//   world is: the in-frame host's game, or a server, for a guest inside its turn (`as_guest`); each player has his own search;
// - the server tells the guest's client to open the list (`SyncPacket` + `GuiWrapper.openGUI`) with what only it knows
//   (the compass FOUND, teleport allowed, creative, levels) over forja's channel `naturescompass:open`;
// - the list is the client's; its buttons go back to the server on `naturescompass:ask` (`CompassSearchPacket`,
//   `SearchForNextPacket`, `TeleportPacket`), which runs them in the guest's turn.
// The HUD reads the compass' data, which the server's inventory sync brings to the client.
#[allow(unused_imports)]
use super::{Dim, Game, Result, Screen, lan::LanHost};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_world::nbt::Tag;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[allow(dead_code)]
mod engine {
    include!("naturescompass_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "naturescompass";

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");

/// The compass' search that runs while the game ticks (`WorldWorkerManager` + `BiomeSearchWorker`).
struct Active {
    worker: engine::Worker,
    terrain: Arc<horizon_client::vanillagen::VanillaTerrain>,
    /// Where the compass was when the search began; searched again if it moved.
    slot: usize,
}
/// The open screen.
struct Modal {
    model: engine::Model,
    search: horizon_client::ui::EditBox,
    /// The compass' slot (`getHeldNatureCompass` at the time of the click).
    last_cursor: (f32, f32),
}
/// A teleport waiting for its chunk to load: `findValidTeleportHeight` reads blocks.
struct Pending {
    x: i32,
    z: i32,
    since: Instant,
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
}
thread_local! {
    // The integrated server and the window run on different threads: each has its own half.
    static ST: RefCell<St> = RefCell::new(St::default());
}
fn with<R>(f: impl FnOnce(&mut St) -> R) -> R {
    ST.with(|s| f(&mut s.borrow_mut()))
}

/// Server -> client: open the list (`SyncPacket` + `openGUI`): the biome of a FOUND compass, (teleport allowed, creative),
/// the player's levels.
type Open = (Option<String>, (bool, bool), i64);
const OPEN: super::forja::Channel<Open> = super::forja::Channel::new("naturescompass:open");
/// Client -> server: a button of the list: (ASK_SEARCH, biome), (ASK_NEXT, ""), (ASK_TELEPORT, "").
const ASK: super::forja::Channel<(u8, String)> = super::forja::Channel::new("naturescompass:ask");
const ASK_SEARCH: u8 = 0;
const ASK_NEXT: u8 = 1;
const ASK_TELEPORT: u8 = 2;

fn nc_register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::forja::register_server_channel(ASK.id, nc_ask_from_client);
}
/// A guest's client pressed a button of the list: it runs as the guest (`as_guest`), as the original's packets run for
/// the `ServerPlayer` who sent them.
fn nc_ask_from_client(game: &mut Game<'_>, host: &mut LanHost, conn: u32, body: &[u8]) {
    let Some((what, biome)) = ASK.decode(body) else { return };
    game.as_guest(host, conn, |g| g.nc_do(what, &biome));
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

/// `string.naturescompass.<key>` in the selected language, the English text of the mod's `en_us.json` last.
fn tr(key: &str) -> String {
    let fallback = match key {
        "status" => "Status",
        "searching" => "Searching",
        "found" => "Found",
        "notFound" => "Not Found",
        "biome" => "Biome",
        "radius" => "Radius",
        "samples" => "Samples",
        "coordinates" => "Coordinates",
        "distance" => "Distance",
        "dimension" => "Dimension",
        "tags" => "Tags",
        "source" => "Source",
        "none" => "None",
        "name" => "Name",
        "levels" => "Levels",
        "rainfall" => "Rainfall",
        "temperature" => "Temperature",
        "search" => "Search",
        "selectBiome" => "Select Biome",
        "startSearch" => "Start Search",
        "searchForNext" => "Search for Next",
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
        biome_key: text(get("BiomeKey")).unwrap_or_default(),
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

/// Writes the data back (the other custom data of the stack stays) and keeps the item model's target:
/// `minecraft:lodestone_tracker` (with `tracked` false, so the game's lodestone check leaves it alone)
/// exists only while the compass is FOUND; the model (`items/naturescompass.json`) points to it then,
/// to the world spawn otherwise (`NaturesCompassClient.getAngle`).
fn write_data(stack: &mut ItemStack, d: &engine::Data, dimension: &str) {
    let mut compound = match stack.data().and_then(|x| x.other("minecraft:custom_data")) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Default::default(),
    };
    let mut put = |k: &str, v: Tag| {
        compound.insert(k.into(), v);
    };
    put("State", Tag::Int(d.state));
    put("BiomeKey", Tag::String(d.biome_key.as_str().into()));
    put("FoundX", Tag::Int(d.found_x));
    put("FoundZ", Tag::Int(d.found_z));
    if let Some(prev) = &d.prev_pos {
        put(
            "PrevPos",
            Tag::List(
                prev.iter()
                    .map(|p| Tag::compound([("X", Tag::Int(p[0])), ("Y", Tag::Int(p[1])), ("Z", Tag::Int(p[2]))]))
                    .collect(),
            ),
        );
    }
    put("Samples", Tag::Int(d.samples));
    put("SearchRadius", Tag::Int(d.search_radius));
    if let Some(b) = d.display_coordinates {
        put("DisplayCoordinates", Tag::Byte(b as i8));
    }
    if d.compass_damage != 0 {
        put("CompassDamage", Tag::Int(d.compass_damage));
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

/// `ServerLevel.dimension()` as the item model compares it.
fn dimension_id(dim: Dim) -> &'static str {
    match dim {
        Dim::Nether => "minecraft:the_nether",
        Dim::End => "minecraft:the_end",
        _ => "minecraft:overworld",
    }
}

/// Wraps the terrain of the dimension the search began in.
struct Terrain(Arc<horizon_client::vanillagen::VanillaTerrain>);
impl engine::BiomeSource for Terrain {
    fn noise_biomes(&mut self, quarts: &[[i32; 3]]) -> Vec<Option<String>> {
        self.0.noise_biomes(quarts)
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

impl Game<'_> {
    /// `ItemUtils.getHeldNatureCompass`: the slot of the compass in the main hand, else in the off hand.
    fn nc_slot(&self) -> Option<usize> {
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

    /// HEAD on `throw_item` (an item used in the air or on a block, after the block's own use):
    /// `NaturesCompassItem.use`. Returns true when it was ours (the original answers PASS).
    ///
    /// A server action: the in-frame host's game opens its own list; a server, in the guest's turn, stops the guest's
    /// search (sneaking) or tells the guest's client to open the list. A server's client only sends the use.
    pub(super) fn naturescompass_use(&mut self, stack: &ItemStack) -> bool {
        if &*stack.item != engine::ITEM || self.remote.is_some() || self.screen != Screen::Playing {
            return false;
        }
        nc_register();
        let Some(slot) = self.nc_slot() else { return false };
        let who = self.lan_acting;
        if kit_blocks::sneaking(self) || (who.is_some() && self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)) {
            // Crouching: the search stops, the compass is inactive.
            with(|st| {
                if let Some(mut a) = st.active.remove(&who) {
                    a.worker.stop();
                }
            });
            self.nc_update(slot, |d| d.state = engine::State::Inactive.id());
            return true;
        }
        let data = read_data(stack);
        if data.is_broken(engine::config::GENERAL_COMPASS_DURABILITY) {
            return true;
        }
        let found = (data.state() == Some(engine::State::Found)).then(|| data.biome_key.clone());
        let can_teleport = engine::config::GENERAL_ALLOW_TELEPORT && self.allow_commands();
        let (creative, levels) = (self.creative, i64::from(self.experience.level));
        match who {
            None => self.nc_show(found, can_teleport, creative, levels),
            // A client without forja (vanilla) has no list to show.
            Some(conn) => {
                if super::forja::guest_has_forja(conn) {
                    super::forja::send_to_acting(self, OPEN.id, &OPEN.encode(&(found, (can_teleport, creative), levels)));
                }
            }
        }
        true
    }

    /// A button of the list, where the world is (the in-frame host, or a server in the guest's turn).
    fn nc_do(&mut self, what: u8, biome: &str) {
        match what {
            ASK_SEARCH => self.nc_search(biome, Vec::new()),
            ASK_NEXT => {
                // `searchForNextBiome`: the biome and the places found are on the compass.
                if let Some(d) = self.nc_slot().and_then(|s| self.nc_data(s))
                    && let Some(prev) = d.prev_pos.clone()
                    && !d.biome_key.is_empty()
                {
                    self.nc_search(&d.biome_key, prev);
                }
            }
            ASK_TELEPORT => self.nc_teleport(),
            _ => {}
        }
    }

    /// The list's button pressed in this window: run here (in-frame host) or sent to the server.
    fn nc_ask(&mut self, what: u8, biome: String) {
        if self.remote.is_some() {
            super::forja::send_to_server(self, ASK.id, &ASK.encode(&(what, biome)));
        } else {
            self.nc_do(what, &biome);
        }
    }

    /// Applies `f` to the data of the compass in `slot` (written back with its model target).
    fn nc_update(&mut self, slot: usize, f: impl FnOnce(&mut engine::Data)) -> bool {
        let dim = dimension_id(self.dim);
        let Some(stack) = self.stack_in_mut(slot).as_mut().filter(|s| &*s.item == engine::ITEM) else {
            return false;
        };
        let mut d = read_data(stack);
        f(&mut d);
        write_data(stack, &d, dim);
        true
    }

    // ---- the list ----------------------------------------------------------------------------

    /// The biome's tags as `BiomeUtils.getBiomeTags` lists them.
    fn nc_tags(&self, key: &str) -> String {
        let mut tags: Vec<&String> = self.res.mob_data().biome_tags.iter().filter(|(_, set)| set.contains(key)).map(|(t, _)| t).collect();
        tags.sort();
        let mut categories: std::collections::BTreeSet<String> = Default::default();
        for tag in tags {
            let (ns, path) = tag.split_once(':').unwrap_or(("minecraft", tag));
            // Only the `is_` tags; `is_overworld` is left out ("some biomes have it and some do not").
            if !path.starts_with("is_") || path == "is_overworld" {
                continue;
            }
            let mut fixed = path.replacen("is_", "", 1);
            if let Some(i) = fixed.find('/') {
                fixed.truncate(i);
            }
            if let Some(t) = lang(&format!("biome.{ns}.{fixed}")) {
                return t;
            }
            if let Some(t) = lang(&format!("category.{ns}.{fixed}")) {
                return t;
            }
            categories.insert(engine::capitalize(&fixed.replace('_', " ")));
        }
        if categories.is_empty() {
            categories.insert(tr("none"));
        }
        categories.into_iter().collect::<Vec<_>>().join(", ")
    }

    /// `BiomeUtils.dimensionKeysToString` over the dimensions that generate the biome (by `is_overworld`, `is_nether`,
    /// `is_end`: what each dimension's biome source lists).
    fn nc_dimensions(&self, key: &str) -> String {
        let tags = &self.res.mob_data().biome_tags;
        let has = |t: &str| tags.get(t).is_some_and(|s| s.contains(key));
        let mut names = Vec::new();
        for (tag, name) in [("minecraft:is_overworld", "Overworld"), ("minecraft:is_nether", "The Nether"), ("minecraft:is_end", "The End")] {
            if has(tag) {
                let id = tag.replace("is_", "dimension.minecraft.").replace("overworld", "overworld");
                let id = match name {
                    "The Nether" => "dimension.minecraft.the_nether".to_owned(),
                    "The End" => "dimension.minecraft.the_end".to_owned(),
                    _ => id,
                };
                names.push(lang(&id).unwrap_or_else(|| name.to_owned()));
            }
        }
        names.join(", ")
    }

    /// The allowed biomes (`BiomeUtils.getAllowedBiomeKeys` without the blacklist) as list rows.
    fn nc_entries(&self) -> Vec<engine::Entry> {
        let mut out = Vec::new();
        for b in &self.res.biomes().list {
            let key = b.name.as_str();
            if engine::blacklisted(engine::config::GENERAL_BIOME_BLACKLIST, key) {
                continue;
            }
            let (ns, path) = key.split_once(':').unwrap_or(("minecraft", key));
            let id = format!("biome.{ns}.{}", path.replace('/', "."));
            let name = horizon_client::lang::get_or(&id, &id);
            out.push(engine::Entry {
                key: key.to_owned(),
                display: engine::fix_biome_name(&name),
                name,
                source: if ns == "minecraft" { "Minecraft".to_owned() } else { ns.to_owned() },
                tags: self.nc_tags(key),
                dimensions: self.nc_dimensions(key),
                xp: engine::xp_levels(key),
                rainfall: b.downfall,
                temperature: b.temperature,
            });
        }
        out
    }

    /// Opens the list over the world in this window (`GuiWrapper.openGUI`) with what the server said: the biome of a FOUND
    /// compass, whether teleporting is allowed, creative (`infiniteXp`), the player's levels.
    fn nc_show(&mut self, found: Option<String>, can_teleport: bool, creative: bool, levels: i64) {
        let model = engine::Model::new(self.nc_entries(), found, can_teleport, engine::config::GENERAL_MAX_NEXT_SEARCHES, creative, levels);
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
    pub(super) fn naturescompass_client_tick(&mut self) {
        // The screen is gone (died, disconnected, another screen took over): the list goes with it.
        if self.screen != Screen::Advancements && with(|st| st.modal.is_some()) {
            with(|st| st.modal = None);
        }
        if self.remote.is_none() {
            return;
        }
        for body in super::forja::client_take(OPEN.id) {
            let Some((found, (can_teleport, creative), levels)) = OPEN.decode(&body) else { continue };
            if self.screen == Screen::Playing {
                self.nc_show(found, can_teleport, creative, levels);
            }
        }
    }

    fn nc_close(&mut self) {
        with(|st| st.modal = None);
        if self.screen == Screen::Advancements {
            self.screen = Screen::Playing;
            self.capture(true);
        }
    }

    fn nc_modal_open(&self) -> bool {
        self.screen == Screen::Advancements && ST.with(|s| s.try_borrow().map(|s| s.modal.is_some()).unwrap_or(true))
    }

    // ---- searching ---------------------------------------------------------------------------

    /// `searchForBiome` / `searchForNextBiome`: the compass is SEARCHING and a worker is made.
    fn nc_search(&mut self, biome: &str, previous: Vec<[i32; 3]>) {
        let Some(slot) = self.nc_slot() else { return };
        if self.nc_data(slot).is_none_or(|d| d.is_broken(engine::config::GENERAL_COMPASS_DURABILITY)) {
            return;
        }
        let Some(p) = self.player.as_ref().map(|p| p.position) else { return };
        let start = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        self.nc_update(slot, |d| d.set_searching(biome));
        let who = self.lan_acting;
        let finish;
        match self.end.locator() {
            Some(terrain) => {
                let mut worker = engine::Worker::new(biome, start, previous, self.dim.min_y(), self.dim.max_y() + 1);
                finish = worker.start();
                with(|st| {
                    // This player's search before it stops (each player has his own, as each has his worker).
                    if let Some(mut a) = st.active.remove(&who) {
                        a.worker.stop();
                    }
                    if finish.is_none() {
                        st.active.insert(who, Active { worker, terrain, slot });
                    }
                });
            }
            // A world without generated terrain (flat, void): nothing to search.
            None => finish = Some(engine::Finish::Failed { radius: 0, samples: 0 }),
        }
        if let Some(f) = finish {
            self.nc_apply(slot, biome, f);
        }
        // `player.giveExperienceLevels(-xpLevels)` unless creative.
        let levels = engine::xp_levels(biome);
        if !self.creative && levels > 0 {
            self.give_levels(-(levels as i32));
        }
    }

    fn nc_data(&self, slot: usize) -> Option<engine::Data> {
        self.stack_in(slot).filter(|s| &*s.item == engine::ITEM).map(read_data)
    }

    /// `NaturesCompassItem.succeed` / `fail`.
    fn nc_apply(&mut self, slot: usize, biome: &str, f: engine::Finish) {
        // The compass may have moved in the inventory meanwhile: the SEARCHING one for this biome.
        let slot = if self.nc_data(slot).is_some() {
            slot
        } else {
            let mut slots: Vec<usize> = (0..self.inventory.len()).collect();
            slots.push(super::shield::OFFHAND_SLOT);
            match slots.into_iter().find(|&s| self.nc_data(s).is_some_and(|d| d.state() == Some(engine::State::Searching) && d.biome_key == biome)) {
                Some(s) => s,
                None => return,
            }
        };
        let durability = engine::config::GENERAL_COMPASS_DURABILITY;
        match f {
            engine::Finish::Found { x, z, prev_pos, samples } => {
                self.nc_update(slot, |d| d.succeed(biome, x, z, prev_pos, samples, engine::config::GENERAL_DISPLAY_COORDINATES, durability));
            }
            engine::Finish::Failed { radius, samples } => {
                self.nc_update(slot, |d| d.fail(biome, radius, samples));
            }
        }
    }

    /// HEAD on `tick_blocks` (once per game tick): the in-frame host's search, teleport and compasses. A server has no
    /// player of its own (`self.player` is `None` on the integrated server): its guests tick in `tick_guests`.
    pub(super) fn naturescompass_tick(&mut self) {
        if self.remote.is_some() || self.lan_acting.is_some() || self.player.is_none() {
            return;
        }
        nc_register();
        self.nc_player_tick();
    }

    /// HEAD on `tick_guests` (the guests' tick on the server; single player's window is a guest): each guest with a search,
    /// a teleport waiting or a compass to point again ticks as the guest.
    pub(super) fn naturescompass_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        nc_register();
        let all: Vec<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        // Who left: his search stops.
        with(|st| {
            st.active.retain(|w, a| {
                let keep = w.is_none_or(|c| all.contains(&c));
                if !keep {
                    a.worker.stop();
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
            self.as_guest(host, conn, |g| g.nc_player_tick());
        }
    }

    /// The tick of the player in place (the in-frame host, or a guest in its turn): the search works for a few milliseconds
    /// (`WorldWorkerManager`), a teleport waiting for its chunk goes on, and the compasses keep their target in this dimension.
    fn nc_player_tick(&mut self) {
        let who = self.lan_acting;
        self.nc_tick_teleport();
        // The compasses in the inventory point in the dimension the player is in.
        let dim = dimension_id(self.dim);
        let mut slots: Vec<usize> = (0..self.inventory.len()).collect();
        slots.push(super::shield::OFFHAND_SLOT);
        for s in slots {
            if self.stack_in(s).is_some_and(|x| stale_target(x, dim)) {
                self.nc_update(s, |_| {});
            }
        }
        let Some(mut active) = with(|st| st.active.remove(&who)) else { return };
        let start = Instant::now();
        let mut src = Terrain(active.terrain.clone());
        let mut done = None;
        while start.elapsed() < Duration::from_millis(8) {
            if let Some(f) = active.worker.do_work(&mut src) {
                done = Some(f);
                break;
            }
        }
        let biome = active.worker.biome.clone();
        let slot = active.slot;
        for r in std::mem::take(&mut active.worker.radius_updates) {
            // `setSearchRadius`, every 500 blocks.
            if self.nc_data(slot).is_some() {
                self.nc_update(slot, |d| d.search_radius = r);
            }
        }
        match done {
            Some(f) => self.nc_apply(slot, &biome, f),
            None => with(|st| {
                st.active.entry(who).or_insert(active);
            }),
        }
    }

    // ---- teleport ----------------------------------------------------------------------------

    /// `TeleportPacket.handle`: allowed by the config and by cheats, FOUND, a free spot at the column.
    fn nc_teleport(&mut self) {
        let Some(slot) = self.nc_slot() else { return };
        if !(engine::config::GENERAL_ALLOW_TELEPORT && self.allow_commands()) {
            return;
        }
        let Some((x, z)) = self.nc_data(slot).and_then(|d| d.target()) else { return };
        if self.world.loaded(x, z) {
            self.nc_teleport_to(x, z);
        } else {
            // Vanilla generates the chunk on the spot; here the player waits for it (a few ticks).
            let who = self.lan_acting;
            with(|st| st.pending.insert(who, Pending { x, z, since: Instant::now() }));
        }
    }
    /// The player in place goes to the column (a guest's client is teleported by `leave_guest`).
    fn nc_teleport_to(&mut self, x: i32, z: i32) {
        let y = engine::teleport_height(&Live { game: self, x, z }, 63);
        self.stop_riding();
        let host = self.lan_acting.is_none();
        if let Some(p) = self.player.as_mut() {
            let v = horizon_core::geometry::Vec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5);
            p.position = v;
            // `if (!player.isFallFlying()) { deltaMovement.y = 0; setOnGround(true) }`
            p.velocity = horizon_core::geometry::Vec3::new(p.velocity.x, 0.0, p.velocity.z);
            p.fall_distance = 0.0;
            p.ground.on_ground = true;
            // The window's interpolation start is the in-frame host's own.
            if host {
                self.previous_position = v;
            }
        }
    }
    fn nc_tick_teleport(&mut self) {
        let who = self.lan_acting;
        let Some((x, z, since)) = with(|st| st.pending.get(&who).map(|p| (p.x, p.z, p.since))) else { return };
        if self.world.loaded(x, z) {
            with(|st| st.pending.remove(&who));
            self.nc_teleport_to(x, z);
        } else if since.elapsed() > Duration::from_secs(20) {
            with(|st| st.pending.remove(&who));
        }
    }

    // ---- HUD ---------------------------------------------------------------------------------

    /// HEAD on `draw_selected_name`: `NaturesCompassOverlay.render`.
    pub(super) fn naturescompass_hud(&self, batch: &mut HudBatch) {
        let show_screen = self.screen == Screen::Playing || (self.screen == Screen::Chat && engine::config::CLIENT_DISPLAY_WITH_CHAT_OPEN);
        if !show_screen || self.debug || self.ui.hide_gui {
            return;
        }
        let Some(slot) = self.nc_slot() else { return };
        let Some(p) = self.player.as_ref().map(|p| p.position) else { return };
        let Some(data) = self.nc_data(slot) else { return };
        let key = &data.biome_key;
        let (ns, path) = key.split_once(':').unwrap_or(("minecraft", key));
        let id = format!("biome.{ns}.{}", path.replace('/', "."));
        let biome_name = horizon_client::lang::get_or(&id, &id);
        let player = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
        let atlas = &self.gui.atlas;
        for l in engine::overlay_lines(&data, player, &biome_name, &tr) {
            let w = HudBatch::text_width(atlas, &l.text);
            let (x, y, shadow) = engine::hud_position(l.rel, batch.width, w);
            let c = rgba((l.color >> 16) as u8, (l.color >> 8) as u8, l.color as u8, 255);
            batch.text(atlas, &l.text, x, y, c, shadow);
        }
    }

    // ---- the screen --------------------------------------------------------------------------

    /// HEAD on `draw_advancements`: the list, in place of the advancements.
    pub(super) fn naturescompass_draw(&mut self, batch: &mut HudBatch) -> bool {
        if !self.nc_modal_open() {
            return false;
        }
        let me: &Game<'_> = self;
        ST.with(|s| {
            if let Ok(mut st) = s.try_borrow_mut()
                && let Some(m) = st.modal.as_mut()
            {
                me.nc_paint(batch, m);
            }
        });
        true
    }

    fn nc_button(&self, batch: &mut HudBatch, r: engine::Rect, label: &str, active: bool) {
        let atlas = &self.gui.atlas;
        let hovered = r.contains(self.cursor.0, self.cursor.1);
        let a = engine::button_alpha(active, hovered);
        batch.fill(atlas, r.x as f32, r.y as f32, r.w as f32, r.h as f32, rgba(0, 0, 0, a));
        let w = HudBatch::text_width(atlas, label);
        batch.text(atlas, label, (r.x as f32 + (r.w as f32 - w) / 2.0).floor(), (r.y + (r.h - 8) / 2) as f32, u32::MAX, true);
    }

    fn nc_paint(&self, batch: &mut HudBatch, m: &mut Modal) {
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width as i32, batch.height as i32);
        // `renderBackground`: the world darkened (-1072689136 -> -804253680).
        batch.fill_gradient(atlas, (0.0, 0.0, w as f32, h as f32), rgba(0x10, 0x10, 0x10, 0xC0), rgba(0x10, 0x10, 0x10, 0xD0));
        let l = m.model.layout(w, h);
        // The title, centred on x = 65.
        let title = tr("selectBiome");
        let tw = HudBatch::text_width(atlas, &title);
        batch.text(atlas, &title, (65.0 - tw / 2.0).floor(), 15.0, u32::MAX, true);
        // Buttons.
        self.nc_button(batch, l.start, &tr("startSearch"), m.model.start_active());
        if let Some(n) = l.next {
            self.nc_button(batch, n, &tr("searchForNext"), m.model.found_selected());
        }
        let label = format!("{}: {}", tr("sortBy"), tr(m.model.sorting.label().0));
        self.nc_button(batch, l.sort, &label, true);
        if let Some(t) = l.teleport {
            self.nc_button(batch, t, &tr("teleport"), m.model.found_selected());
        }
        self.nc_button(batch, l.cancel, &horizon_client::lang::get_or("gui.cancel", "Cancel"), true);
        // The search field (`TransparentTextField`): a black 50% box; the label while empty and unfocused.
        let s = l.search;
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
        // Dragging the scrollbar.
        let list = l.list;
        if m.model.scroll_drag {
            m.model.drag_scroll(self.cursor.1 - m.last_cursor.1, list);
        }
        m.last_cursor = self.cursor;
        // The rows, cut to the list.
        let start = batch.vertices.len();
        let sorting = m.model.sorting;
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
            self.nc_row(batch, entry, sorting, left, top, list.w, enabled);
        }
        batch.clip_since(start, list.x as f32, list.y as f32, (list.x + list.w) as f32, (list.y + list.h) as f32);
        // The scrollbar.
        if let Some((top, th)) = m.model.thumb(list) {
            let x = (list.x + list.w) as f32;
            batch.fill(atlas, x, list.y as f32, engine::SCROLLBAR_WIDTH as f32, list.h as f32, rgba(0, 0, 0, engine::row_alpha(false, false)));
            batch.fill(atlas, x, top as f32, engine::SCROLLBAR_WIDTH as f32, th as f32, rgba(0, 0, 0, engine::row_alpha(true, true)));
        }
    }

    /// `BiomeSearchEntry.render`.
    #[allow(clippy::too_many_arguments)]
    fn nc_row(&self, batch: &mut HudBatch, e: &engine::Entry, sorting: engine::Sorting, left: i32, top: i32, width: i32, enabled: bool) {
        let atlas = &self.gui.atlas;
        let (title, value) = if sorting.shows_dimension() { (tr("dimension"), e.dimensions.clone()) } else { (tr(sorting.label().0), sorting.value(e)) };
        let mut max_text = width - 10;
        let xp = e.xp.min(3);
        if xp > 0 {
            // The orb sprites are textures of the mod, which a native mod cannot draw: the level count in green.
            let c = if enabled { rgba(0x80, 0xFF, 0x20, 255) } else { rgba(0x40, 0x7F, 0x10, 255) };
            let t = format!("{xp}");
            let tw = HudBatch::text_width(atlas, &t);
            batch.text(atlas, &t, (left + width - 20) as f32 - tw / 2.0 + 8.0, (top + 21) as f32, c, true);
            max_text = width - ENTRY_HEIGHT - 5;
        }
        let mut tags_line = format!("{}: {}", tr("tags"), e.tags);
        if HudBatch::text_width(atlas, &tags_line) > max_text as f32 {
            let mut cut = String::new();
            for ch in format!("{tags_line}...").chars() {
                let next = format!("{cut}{ch}");
                if HudBatch::text_width(atlas, &next) > max_text as f32 {
                    break;
                }
                cut = next;
            }
            tags_line = format!("{cut}...");
        }
        let name_color = if enabled { u32::MAX } else { rgba(0x80, 0x80, 0x80, 255) };
        let info_color = if enabled { rgba(0x80, 0x80, 0x80, 255) } else { rgba(0x55, 0x55, 0x55, 255) };
        let line = 11; // font.lineHeight + 2
        let mid = top + ENTRY_HEIGHT / 2;
        batch.text(atlas, &e.display, (left + 5) as f32, (mid - line * 2) as f32, name_color, true);
        batch.text(atlas, &format!("{title}: {value}"), (left + 5) as f32, (mid - line) as f32, info_color, true);
        batch.text(atlas, &tags_line, (left + 5) as f32, mid as f32, info_color, true);
        batch.text(atlas, &format!("{}: {}", tr("source"), e.source), (left + 5) as f32, (mid + line) as f32, info_color, true);
    }

    /// HEAD on `advancements_mouse`: a press (`pressed`) or release of the left button on the list.
    pub(super) fn naturescompass_mouse(&mut self, pressed: bool, size: [u32; 2]) -> bool {
        if !self.nc_modal_open() {
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
            // A click outside the field takes the focus away; on it, gives it.
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
            engine::Action::Close => self.nc_close(),
            // `CompassSearchPacket`, `SearchForNextPacket`, `TeleportPacket`: to the server (or here, in frame).
            engine::Action::Search(biome) => {
                self.nc_close();
                self.nc_ask(ASK_SEARCH, biome);
            }
            engine::Action::SearchNext => {
                self.nc_close();
                self.nc_ask(ASK_NEXT, String::new());
            }
            engine::Action::Teleport => {
                self.nc_close();
                self.nc_ask(ASK_TELEPORT, String::new());
            }
        }
        true
    }

    /// HEAD on `advancements_drag` (the pointer moved): the scrollbar follows in the next frame.
    pub(super) fn naturescompass_drag(&mut self) -> bool {
        self.nc_modal_open()
    }

    /// HEAD on `advancements_scroll`: the wheel turns the list.
    pub(super) fn naturescompass_scroll(&mut self, delta: f32) -> bool {
        if !self.nc_modal_open() {
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

    /// HEAD on `sort_key_press`: every key belongs to the list while it is open (Escape closes it).
    pub(super) fn naturescompass_key(&mut self, key: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode as K;
        if !self.nc_modal_open() {
            return false;
        }
        if key == K::Escape {
            self.nc_close();
            return true;
        }
        // The screenshot, fullscreen and hide-GUI keys still work.
        !matches!(key, K::F1 | K::F2 | K::F11)
    }

    /// HEAD on `recipe_search_focused`: typed keys go to the field while it has the focus.
    pub(super) fn naturescompass_typing(&self) -> bool {
        self.nc_modal_open() && ST.with(|s| s.try_borrow().map(|s| s.modal.as_ref().is_some_and(|m| m.search.focused)).unwrap_or(false))
    }

    /// HEAD on `recipe_key`: a key typed into the field (`keyPressed` / `charTyped` + `processSearchTerm`).
    pub(super) fn naturescompass_type(&mut self, event: &winit::event::KeyEvent) -> bool {
        use winit::keyboard::{KeyCode as K, PhysicalKey};
        if !self.naturescompass_typing() {
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

    /// HEAD on `load_world_state`: a world opens, nothing of the one before stays.
    pub(super) fn naturescompass_world_opened() {
        with(|st| *st = St::default());
    }
}

/// `BiomeSearchList` slot height.
const ENTRY_HEIGHT: i32 = engine::ROW_HEIGHT;
