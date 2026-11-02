// Hats Renewed inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs).
//
// Single player runs the world on the integrated server's thread and the window joins it through memory, in one
// process. The mod's world state (`HatsSavedData`: collections, worn and thrown hats) lives in one process-wide
// `Shared`, written by the side that owns the world (spawn rolls, deaths, launches, saving) and read by the window
// (drawing, the menu, the toast), as the original's packets would carry it. A client of another PC (LAN guest,
// remote server) gets none of it: see README.
#[allow(unused_imports)]
use super::{Game, Result};
#[allow(dead_code)]
mod engine {
    include!("hats_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
const MOD_ID: &str = "hats";

include!("kit_blocks.rs");
include!("kit_sound.rs");
include!("kit_state.rs");

use horizon_client::{hud::HudBatch, items::ItemStack, mesher::Vertex, mobs::Mob, text::Text};
use std::collections::HashMap;

pub const ID: &str = "hats";
pub const TITLE: &str = "Hats Renewed";
pub const DESCRIPTION: &str = "Mobs spawn wearing hats (the pack's chances: common 10%, uncommon 8%, rare 5%, epic 3%, \
legendary 2%); kill one to unlock its hat. H opens your collection: pick a hat and its accessories to wear. The Hat \
Launcher throws a hat from your collection onto a mob (sneak + use to pick which).";
const LAUNCHER: &str = "hats:hat_launcher";
const SAVE_FILE: &str = "hats_save.json";

/// `HatsSavedData` and what travels between the world's side and the window's.
#[derive(Default)]
struct Shared {
    /// The world folder the save belongs to (`None`: not loaded yet).
    world: Option<std::path::PathBuf>,
    save: engine::Save,
    thrown: Vec<engine::Thrown>,
    dirty: bool,
    /// The world's entity id -> the hat it wears (mobs; players wear their equipped hat), published every tick.
    by_id: HashMap<u32, engine::HatPart>,
    /// "New Hat Unlocked!" toasts for a player's window: (player UUID, unlock).
    unlocks: Vec<(String, engine::Unlock)>,
    /// The hat a player picked to launch next (`HatLaunchSelectScreen.pendingHatName`).
    launch_name: HashMap<String, String>,
    /// Hits to show on the window: (position, height) of the struck entity (`spawnPickupParticles`).
    poofs: Vec<[f64; 3]>,
}

static SHARED: std::sync::Mutex<Option<Shared>> = std::sync::Mutex::new(None);

fn shared<R>(f: impl FnOnce(&mut Shared) -> R) -> R {
    let mut g = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Shared::default))
}

/// The window's own state: the menu, the toasts, per-frame counters, the head boxes per mob kind.
#[derive(Default)]
struct Window {
    menu: Option<Menu>,
    /// Toasts waiting or shown: (unlock, the time it took its slot).
    toasts: std::collections::VecDeque<(engine::Unlock, Option<std::time::Instant>)>,
    /// The toast's slide sounds from the drawing pass, played by the next frame's first hook.
    sounds: Vec<&'static str>,
    toast_sound_state: (bool, bool),
    drawn: usize,
    heads: HashMap<String, Option<([f32; 3], f32)>>,
    /// The hat picked for the next launch (window side, moved into `Shared` on use).
    pending_launch: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum Mode {
    Browse,
    Launch,
}

#[derive(Clone, Debug)]
struct Menu {
    mode: Mode,
    scroll: usize,
    selected: Option<String>,
    sort: engine::Sort,
}

thread_local! {
    static WINDOW: std::cell::RefCell<Window> = std::cell::RefCell::new(Window::default());
    /// The rolled hat of each UUID (a pure function of it: cached).
    static ROLLED: std::cell::RefCell<HashMap<[i32; 4], Option<engine::HatPart>>> = std::cell::RefCell::new(HashMap::new());
}

fn window<R>(f: impl FnOnce(&mut Window) -> R) -> R {
    WINDOW.with(|w| f(&mut w.borrow_mut()))
}

/// The hats of the carry (`assets/hats/horizon/hats-index.json` in the content root), loaded once.
fn registry(root: &std::path::Path) -> &'static engine::Registry {
    static REG: std::sync::OnceLock<engine::Registry> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        std::fs::read(root.join("assets/hats/horizon/hats-index.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .map(|v| engine::Registry::from_index(&v))
            .unwrap_or_default()
    })
}

fn uuid_text(u: [i32; 4]) -> String {
    engine::uuid_string(u)
}

/// A hat drawn as an item: the carry's flat item definition `hats:hat.<key>` (its model `hats:item/hat/<key>`).
fn model_view(def: &engine::HatDef) -> horizon_assets::item_model::StackView {
    let id = def.item.clone();
    let mut view = horizon_assets::item_model::StackView::new(&id, 1);
    view.patch.insert("minecraft:item_model".to_owned(), Some(serde_json::Value::String(id)));
    view
}

impl Game<'_> {
    fn hats_reg(&self) -> &'static engine::Registry {
        registry(&self.options.resources)
    }

    // ---- the world's side ----------------------------------------------------------------------------------

    /// The save of this world, read once from `<world>/data/hats/hats_save.json`.
    fn hats_ensure_loaded(&self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        let root = self.world_root.clone();
        if shared(|s| s.world.as_ref() == Some(&root)) {
            return;
        }
        let text = kit_state::read(self, SAVE_FILE);
        shared(|s| {
            *s = Shared::default();
            s.world = Some(root);
            if let Some(v) = text.and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
                s.save = engine::Save::from_json(&v);
                s.thrown = v["thrown"].as_array().into_iter().flatten().filter_map(engine::Thrown::from_json).collect();
            }
        });
    }

    /// The UUID of the player the world acts for: a network player's in its turn (single player's window is one),
    /// else the world's own player.
    fn hats_actor_uuid(&self) -> Option<[i32; 4]> {
        if let Some(conn) = self.lan_acting {
            let host = self.lan.as_ref()?;
            let g = host.hub.guests.iter().find(|g| g.conn == conn)?;
            return Some(engine::uuid_ints(g.uuid));
        }
        self.level.player_uuid
    }

    /// The hat an entity of the world wears: an entry of `entityHats`, else its spawn roll.
    fn hats_worn(&self, uuid: [i32; 4], kind: &str) -> Option<engine::HatPart> {
        let key = uuid_text(uuid);
        if let Some(entry) = shared(|s| s.save.entity_hats.get(&key).cloned()) {
            return entry;
        }
        let reg = self.hats_reg();
        ROLLED.with(|r| r.borrow_mut().entry(uuid).or_insert_with(|| engine::roll_mob_hat(reg, uuid, kind)).clone())
    }

    /// HEAD `creatures::Game::tick_creatures` (the world's tick): `assignMobHat` for the mobs (published by id
    /// for the window), then `HatEntity.tick` for the thrown hats.
    pub(super) fn hats_world_tick(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        self.hats_ensure_loaded();
        let mobs: Vec<(u32, String)> = self
            .mobs
            .iter()
            .filter(|m| !m.removed && &*m.kind.id != "minecraft:player")
            .map(|m| (m.id, m.kind.id.to_string()))
            .collect();
        let mut by_id = HashMap::new();
        for (id, kind) in &mobs {
            let uuid = self.uuid_for(*id);
            if let Some(h) = self.hats_worn(uuid, kind) {
                by_id.insert(*id, h);
            }
        }
        shared(|s| s.by_id = by_id);
        self.hats_tick_thrown();
    }

    fn hats_tick_thrown(&mut self) {
        if shared(|s| s.thrown.is_empty()) {
            return;
        }
        // The living entities a hat can land on: mobs and the players.
        let mut targets: Vec<engine::Target> = Vec::new();
        let mut player_of_target: Vec<bool> = Vec::new();
        let ids: Vec<u32> = self.mobs.iter().filter(|m| !m.removed && m.alive()).map(|m| m.id).collect();
        let mut mob_ids = Vec::new();
        for id in ids {
            let uuid = self.uuid_for(id);
            let Some(m) = self.mobs.iter().find(|m| m.id == id) else { continue };
            let b = m.bounds();
            let (lo, hi) = (b.min_position(), b.max_position());
            targets.push(engine::Target {
                uuid: uuid_text(uuid),
                min: [lo.x, lo.y, lo.z],
                max: [hi.x, hi.y, hi.z],
                spectator: false,
            });
            player_of_target.push(&*m.kind.id == "minecraft:player");
            mob_ids.push(Some((id, m.kind.id.to_string())));
        }
        if let Some(host) = &self.lan {
            for (conn, m) in &host.models {
                let Some(g) = host.hub.guests.iter().find(|g| g.conn == *conn) else { continue };
                let b = m.bounds();
                let (lo, hi) = (b.min_position(), b.max_position());
                targets.push(engine::Target {
                    uuid: uuid_text(engine::uuid_ints(g.uuid)),
                    min: [lo.x, lo.y, lo.z],
                    max: [hi.x, hi.y, hi.z],
                    spectator: false,
                });
                player_of_target.push(true);
                mob_ids.push(None);
            }
        }
        if let (Some(p), Some(u)) = (self.player.as_ref(), self.level.player_uuid) {
            let (x, y, z) = (p.position.x, p.position.y, p.position.z);
            targets.push(engine::Target {
                uuid: uuid_text(u),
                min: [x - 0.3, y, z - 0.3],
                max: [x + 0.3, y + 1.8, z + 0.3],
                spectator: false,
            });
            player_of_target.push(true);
            mob_ids.push(None);
        }
        let world = Solid(self);
        let mut thrown = shared(|s| std::mem::take(&mut s.thrown));
        let mut keep = Vec::new();
        let mut hits = Vec::new();
        for mut t in thrown.drain(..) {
            match engine::tick_thrown(&mut t, &world, &targets) {
                engine::Step::Flying => keep.push(t),
                engine::Step::Expired => {}
                engine::Step::Hit(i) => hits.push((t, i)),
            }
        }
        let mut rand = engine::JavaRandom::new(self.ticks as i64);
        for (t, i) in hits {
            let target = &targets[i];
            let center = [
                (target.min[0] + target.max[0]) / 2.0,
                target.min[1] + (target.max[1] - target.min[1]) * 0.5,
                (target.min[2] + target.max[2]) / 2.0,
            ];
            let pitch = 0.85 + rand.next_float() * 0.3;
            kit_sound::at(self, "hats:poof", [center[0], target.min[1], center[2]], 0.6, pitch);
            if player_of_target[i] {
                // A player catches it: into the collection (no toast, as `hitEntity`).
                shared(|s| {
                    s.save.player(&target.uuid).add_hat(t.hat.clone());
                    s.poofs.push(center);
                    s.dirty = true;
                });
                continue;
            }
            let existing = match &mob_ids[i] {
                Some((id, kind)) => {
                    let uuid = self.uuid_for(*id);
                    self.hats_worn(uuid, kind)
                }
                None => None,
            };
            let (worn, bounced) = engine::hit_mob(&t, existing, &target.uuid, &mut rand);
            shared(|s| {
                s.save.entity_hats.insert(target.uuid.clone(), Some(worn));
                s.poofs.push(center);
                s.dirty = true;
            });
            if let Some(b) = bounced {
                keep.push(b);
            }
        }
        shared(|s| {
            keep.append(&mut s.thrown);
            s.thrown = keep;
            s.dirty = true;
        });
    }

    /// HEAD `creatures::Game::mob_died`: `onEntityDeath`. The mob's hat leaves it; killed by a player, the hat is
    /// unlocked in that player's collection (`onPlayerPickupHat`).
    pub(super) fn hats_mob_died(&mut self, m: &Mob) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() || &*m.kind.id == "minecraft:player" {
            return;
        }
        self.hats_ensure_loaded();
        let uuid = self.uuid_for(m.id);
        let Some(hat) = self.hats_worn(uuid, &m.kind.id) else {
            return;
        };
        shared(|s| {
            s.save.entity_hats.insert(uuid_text(uuid), None);
            s.dirty = true;
        });
        // source.getEntity() instanceof ServerPlayer: the killing blow came from a player.
        let by_player = m
            .last_damage
            .as_ref()
            .is_some_and(|(d, _)| d.attacker == Some(horizon_client::mobs::Who::Player));
        if !by_player {
            return;
        }
        let player = match self.last_hurt_by_player.get(&m.id).copied().flatten().or(self.lan_acting) {
            Some(conn) => self
                .lan
                .as_ref()
                .and_then(|h| h.hub.guests.iter().find(|g| g.conn == conn))
                .map(|g| engine::uuid_ints(g.uuid)),
            None => self.level.player_uuid,
        };
        let Some(player) = player else {
            return;
        };
        let key = uuid_text(player);
        shared(|s| {
            let unlocks = s.save.pickup(&key, hat);
            for u in unlocks {
                s.unlocks.push((key.clone(), u));
            }
            s.dirty = true;
        });
    }

    /// HEAD `creatures::Game::throw_item` (`Item.use` in the air or on a block) for the Hat Launcher, on the side
    /// that owns the world: `PacketLaunchHat` (a hat from the collection, the one picked or a random non-favourite,
    /// thrown from the eyes with a bonk and a 10 tick cooldown). `Some(swing)` = handled.
    pub(super) fn hats_launch(&mut self, stack: &ItemStack) -> Option<bool> {
        if &*stack.item != LAUNCHER {
            return None;
        }
        if self.remote.is_some() {
            return None;
        }
        // A world in this window: sneaking opens the launch screen here (client side of `HatLauncherItem.use`).
        if self.lan_acting.is_none() && kit_blocks::sneaking(self) {
            self.hats_open_menu(Mode::Launch);
            return Some(true);
        }
        if self.lan_acting.is_none()
            && let Some(name) = window(|w| w.pending_launch.take())
            && let Some(u) = self.level.player_uuid
        {
            shared(|s| s.launch_name.insert(uuid_text(u), name));
        }
        self.hats_ensure_loaded();
        let uuid = self.hats_actor_uuid()?;
        let key = uuid_text(uuid);
        let (Some(eye), look) = (self.eye_position(), super::view_vector(self.yaw, self.pitch)) else {
            return Some(true);
        };
        let hat = shared(|s| {
            let pending = s.launch_name.remove(&key);
            let data = s.save.player(&key);
            let hat = match pending.filter(|n| !n.is_empty()) {
                Some(name) => data.take_specific(&name),
                None => {
                    // takeRandomNonFavourite: RAND seeded with the player's UUID each time.
                    let seed = i64::from(engine::java_hash(&format!("{}{key}", engine::config::RAND_SEED)).wrapping_abs())
                        .wrapping_mul(425_480_085);
                    data.take_random_non_favourite(&mut engine::JavaRandom::new(seed))
                }
            };
            if hat.is_some() {
                s.dirty = true;
            }
            hat
        });
        let Some(hat) = hat else {
            return Some(true);
        };
        let mut rand = engine::JavaRandom::new(self.ticks as i64 ^ 0x5DEE_CE66D);
        let t = engine::launch(hat, eye, look, &key, &mut rand);
        shared(|s| s.thrown.push(t));
        let feet = self.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]).unwrap_or(eye);
        let pitch = 0.9 + rand.next_float() * 0.2;
        kit_sound::at(self, "hats:bonk", feet, 0.8, pitch);
        // getCooldowns().addCooldown(HAT_LAUNCHER, 10)
        let group: std::sync::Arc<str> = std::sync::Arc::from(LAUNCHER);
        self.cooldowns.add(std::sync::Arc::clone(&group), 10);
        if self.lan_acting.is_some() {
            self.guest_cooldowns.push((group, 10));
        }
        Some(true)
    }

    /// HEAD `save_game_metadata`: `HatsSavedData` is written with the world.
    pub(super) fn hats_save(&mut self) {
        if self.remote.is_some() || self.world_root.as_os_str().is_empty() {
            return;
        }
        let root = self.world_root.clone();
        let text = shared(|s| {
            if s.world.as_ref() != Some(&root) || !s.dirty {
                return None;
            }
            s.dirty = false;
            let mut v = s.save.to_json();
            v["thrown"] = serde_json::Value::Array(s.thrown.iter().map(engine::Thrown::to_json).collect());
            Some(v.to_string())
        });
        if let Some(text) = text {
            kit_state::write(self, SAVE_FILE, &text);
        }
    }

    /// HEAD `load_world_state` (no bindings): a world opens; its save is read on first use.
    pub(super) fn hats_world_opened() {
        shared(|s| *s = Shared::default());
        ROLLED.with(|r| r.borrow_mut().clear());
    }

    // ---- the window's side ---------------------------------------------------------------------------------

    /// The window player's UUID as the world knows it.
    fn hats_own_uuid(&self) -> Option<[i32; 4]> {
        match &self.remote {
            Some(r) => Some(engine::uuid_ints(r.profile())),
            None => self.level.player_uuid,
        }
    }

    /// The hat a drawn entity wears.
    fn hats_on(&self, mob: &Mob) -> Option<engine::HatPart> {
        if &*mob.kind.id == "minecraft:player" {
            let local = self.player_model.as_ref().is_some_and(|p| p.id == mob.id && p.kind.id == mob.kind.id);
            let uuid = if local { self.hats_own_uuid() } else { self.persist.uuids.get(&mob.id).copied() }?;
            let key = uuid_text(uuid);
            return shared(|s| s.save.players.get(&key).and_then(|p| p.equipped.clone()));
        }
        shared(|s| s.by_id.get(&mob.id).cloned())
    }

    /// The main box of a mob kind's head (`LayerHat.computeHeadOffsets`), cached per kind.
    fn hats_head_of(&self, mob: &Mob) -> Option<([f32; 3], f32)> {
        if let Some(c) = window(|w| w.heads.get(&*mob.kind.id).copied()) {
            return c;
        }
        fn find<'a>(p: &'a horizon_client::mobs::Part) -> Option<&'a horizon_client::mobs::Part> {
            if p.name == "head" {
                return Some(p);
            }
            p.children.iter().find_map(find)
        }
        let head = find(&mob.kind.model).and_then(|h| {
            let quads: Vec<[[f32; 3]; 4]> = h
                .polygons
                .iter()
                .map(|p| p.vertices.map(|v| [v[0], v[1], v[2]]))
                .collect();
            engine::head_offsets(&engine::boxes_from_quads(&quads))
        });
        window(|w| w.heads.insert(mob.kind.id.to_string(), head));
        head
    }

    /// Draw one worn hat (with its showing accessories) under `frame`, the top-centre of a head of a player's
    /// size; `CustomHeadLayer`'s pose is where the content tool placed the item models.
    fn hats_draw_part(&self, part: &engine::HatPart, frame: glam::Mat4, owner: &horizon_assets::item_model::OwnerState, seed: i32, light: u32, out: &mut Vec<Vertex>) {
        use horizon_assets::item_model::DisplayContext;
        let reg = self.hats_reg();
        let mut defs = Vec::new();
        engine::drawn_parts(reg, part, &mut defs);
        let pose = frame
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.5, 0.0))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, -0.25, 0.0))
            * glam::Mat4::from_rotation_y(std::f32::consts::PI)
            * glam::Mat4::from_scale(glam::Vec3::new(0.625, -0.625, -0.625));
        let level = self.item_level();
        for def in defs {
            let view = model_view(def);
            let state = self.pack.item_renderer.resolve(&view, DisplayContext::Head, Some(owner), Some(&level), seed, &self.item_env());
            self.submit_item_world(&state, pose, light, out);
        }
    }

    /// HEAD `mob_blocks::Game::mob_block_geometry` (every drawn mob and player): `LayerHat`.
    pub(super) fn hats_draw_mob(&self, mob: &Mob, camera: glam::DVec3, alpha: f64, light: u32, out: &mut Vec<Vertex>) {
        if horizon_client::mobs::trader::is_invisible(mob) && !engine::config::HATS_SHOW_WHEN_INVISIBLE {
            return;
        }
        let Some(hat) = self.hats_on(mob) else {
            return;
        };
        if window(|w| {
            w.drawn += 1;
            w.drawn > engine::config::MAX_HATS_ON_SCREEN
        }) {
            return;
        }
        let Some(head) = horizon_client::mobs::render::part_transform(mob, alpha as f32, camera, self.res.mob_data(), "head") else {
            return;
        };
        let (centre, geometry) = self.hats_head_of(mob).unwrap_or(([0.0, -0.5, 0.0], 1.0));
        let (placement, _) = engine::placement(&mob.kind.id);
        let frame = head
            * glam::Mat4::from_translation(glam::Vec3::from_array(centre))
            * glam::Mat4::from_scale(glam::Vec3::splat(placement * geometry));
        let mut owner = super::item_draw::owner_at([mob.position.x, mob.position.y, mob.position.z], mob.body_yaw, &mob.kind.id);
        owner.main_arm_left = mob.main_arm_left();
        self.hats_draw_part(&hat, frame, &owner, mob.id as i32 + 5, light, out);
    }

    /// HEAD `names::Game::name_tag_geometry` (world drawing with the partial tick): the thrown hats
    /// (`HatEntityRenderer`), spinning by their rotation factors.
    pub(super) fn hats_draw_thrown(&self, camera: glam::DVec3, alpha: f64, out: &mut Vec<Vertex>) {
        let thrown: Vec<engine::Thrown> = shared(|s| s.thrown.clone());
        for t in thrown {
            let p: [f64; 3] = std::array::from_fn(|i| t.prev_pos[i] + (t.pos[i] - t.prev_pos[i]) * alpha);
            let rot: [f32; 2] = std::array::from_fn(|i| t.prev_rot[i] + (t.rot[i] - t.prev_rot[i]) * alpha as f32);
            let b = [p[0].floor() as i32, (p[1] + 0.25).floor() as i32, p[2].floor() as i32];
            let l = u32::from(self.world.light(b[0], b[1], b[2]));
            let light = ((l & 15) << 4) | (((l >> 4) & 15) << 4) << 8;
            let frame = glam::Mat4::from_translation(glam::Vec3::new(
                (p[0] - camera.x) as f32,
                (p[1] - camera.y) as f32,
                (p[2] - camera.z) as f32,
            )) * glam::Mat4::from_rotation_y(rot[1].to_radians())
                * glam::Mat4::from_rotation_x(rot[0].to_radians())
                * glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
            let owner = super::item_draw::owner_at(p, 0.0, "hats:hat");
            let mut hat = t.hat.clone();
            hat.showing = true;
            self.hats_draw_part(&hat, frame, &owner, 0, light, out);
        }
    }

    /// HEAD `remote::Game::remote_use_hands` (a right click of the window that reached `Item.use`): a thrown hat
    /// under the cross-hair is picked up (`HatEntity.interact`); the Hat Launcher with sneak opens the launch
    /// screen, without it the hat picked there goes with the use. `true` = the click is taken.
    pub(super) fn hats_window_use(&mut self) -> bool {
        let Some(uuid) = self.hats_own_uuid() else {
            return false;
        };
        let key = uuid_text(uuid);
        if let Some(eye) = self.eye_position() {
            let look = super::view_vector(self.yaw, self.pitch);
            let reach = if self.creative { 5.0 } else { 4.5 };
            let taken = shared(|s| {
                let i = engine::picked(&s.thrown, eye, look, reach)?;
                let t = s.thrown.remove(i);
                s.save.player(&key).add_hat(t.hat);
                s.dirty = true;
                Some(())
            });
            if taken.is_some() {
                return true;
            }
        }
        let held = self.inventory[self.selected].as_ref().is_some_and(|s| &*s.item == LAUNCHER);
        if !held {
            return false;
        }
        if self.has_shift_down() || kit_blocks::sneaking(self) {
            self.hats_open_menu(Mode::Launch);
            return true;
        }
        if let Some(name) = window(|w| w.pending_launch.take()) {
            shared(|s| s.launch_name.insert(key, name));
        }
        false
    }

    /// HEAD `scoreboard::Game::draw_tab_list` (every HUD frame, before the screens): per-frame counters, the new
    /// toasts of this window's player, the toast sounds, hits' particles, and the menu closing with its screen.
    pub(super) fn hats_frame(&mut self) {
        let own = self.hats_own_uuid().map(uuid_text);
        let (unlocks, poofs) = shared(|s| {
            let mine: Vec<engine::Unlock> = match &own {
                Some(k) => {
                    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.unlocks).into_iter().partition(|(u, _)| u == k);
                    s.unlocks = rest;
                    mine.into_iter().map(|(_, u)| u).collect()
                }
                None => Vec::new(),
            };
            (mine, std::mem::take(&mut s.poofs))
        });
        let sounds = window(|w| {
            w.drawn = 0;
            if engine::config::DISPLAY_HAT_UNLOCK_TOAST {
                w.toasts.extend(unlocks.into_iter().map(|u| (u, None)));
            }
            if w.menu.is_some() && self.screen != super::Screen::Advancements {
                w.menu = None;
            }
            std::mem::take(&mut w.sounds)
        });
        for s in sounds {
            self.audio.play_unshared(s, None, 1.0, 1.0);
        }
        // spawnPickupParticles: 8 poofs (speed 0.05) and 5 electric sparks (0.1) around the struck entity.
        use horizon_client::particles::{ParticleOptions, ParticleType};
        for c in poofs {
            let mut r = engine::JavaRandom::new(self.ticks as i64);
            for (ty, n, spread, speed) in [(ParticleType::Poof, 8, 0.6, 0.05), (ParticleType::ElectricSpark, 5, 0.5, 0.1)] {
                for _ in 0..n {
                    let at = [
                        c[0] + (r.next_double() - 0.5) * spread,
                        c[1] + (r.next_double() - 0.5) * spread,
                        c[2] + (r.next_double() - 0.5) * spread,
                    ];
                    self.particles.send_particles(&ParticleOptions::simple(ty), at, [0.0; 3], speed, 1);
                }
            }
        }
    }

    /// HEAD `remote::Game::remote_hud` (the HUD's last layer): the "New Hat Unlocked!" toasts (`HatToast`) in the
    /// first toast slot the game's toasts leave free.
    pub(super) fn hats_draw_toasts(&self, batch: &mut HudBatch) {
        if self.ui.hide_gui {
            return;
        }
        let used = self.toast_slots_used();
        let Some(slot) = (0..5).find(|s| used & (1 << s) == 0) else {
            return;
        };
        let Some((unlock, start)) = window(|w| {
            let (u, start) = w.toasts.front_mut()?;
            let st = *start.get_or_insert_with(std::time::Instant::now);
            Some((u.clone(), st))
        }) else {
            return;
        };
        let ms = start.elapsed().as_millis() as i64;
        let (portion, finished) = engine::toast_slide(ms);
        window(|w| {
            // ToastInstance: the slide-in and slide-out sounds.
            let (shown, hidden) = w.toast_sound_state;
            if !shown {
                w.sounds.push("minecraft:ui.toast.in");
            }
            if ms >= 5600 && !hidden {
                w.sounds.push("minecraft:ui.toast.out");
            }
            w.toast_sound_state = (true, ms >= 5600);
            if finished {
                w.toasts.pop_front();
                w.toast_sound_state = (false, false);
            }
        });
        if finished {
            return;
        }
        let atlas = &self.gui.atlas;
        let x = batch.width - 160.0 * portion;
        let y = 32.0 * slot as f32;
        batch.fill(atlas, x, y, 160.0, 32.0, 0xFFFF_FFFF);
        batch.fill(atlas, x, y, 160.0, 1.0, 0xFFCC_CCCC);
        batch.fill(atlas, x, y + 31.0, 160.0, 1.0, 0xFFAA_AAAA);
        batch.fill(atlas, x, y, 1.0, 32.0, 0xFFCC_CCCC);
        batch.fill(atlas, x + 159.0, y, 1.0, 32.0, 0xFFAA_AAAA);
        let title = if unlock.accessory { "hats.toast.new_accessory" } else { "hats.toast.new_hat" };
        batch.text_styled(atlas, &Text::translate(title, &[]), x + 36.0, y + 7.0, 0xFF33_3333, false);
        batch.text(atlas, &unlock.hat.name, x + 36.0, y + 18.0, 0xFF55_5555, false);
        let reg = self.hats_reg();
        if let Some(def) = reg.def_of(&unlock.hat).or_else(|| reg.get(&unlock.hat.name)) {
            self.hats_icon(batch, def, x + 4.0, y + 4.0, 24.0);
        }
    }

    /// A hat's item model in the GUI, `size` px wide.
    fn hats_icon(&self, batch: &mut HudBatch, def: &engine::HatDef, x: f32, y: f32, size: f32) {
        use horizon_assets::item_model::DisplayContext;
        let owner = self.player_owner(None);
        let state = self.pack.item_renderer.resolve(&model_view(def), DisplayContext::Gui, Some(&owner), Some(&self.item_level()), 0, &self.item_env());
        let start = batch.vertices.len();
        horizon_client::item_render::gui::draw(
            &state,
            &self.pack.item_renderer,
            &self.gui.atlas.sprites,
            &self.item_host(),
            batch,
            x,
            y,
            self.started.elapsed().as_secs_f32(),
            self.settings.file.double("glintSpeed") as f32,
            self.settings.file.double("glintStrength") as f32,
        );
        if size != 16.0 {
            batch.scale_since(start, x, y, size / 16.0, size / 16.0);
        }
    }

    // ---- the menu (`HatsScreen`, on the Advancements screen) -------------------------------------------------

    fn hats_open_menu(&mut self, mode: Mode) {
        if self.remote.is_none() && self.lan_acting.is_some() {
            return;
        }
        window(|w| {
            w.menu = Some(Menu {
                mode,
                scroll: 0,
                selected: None,
                sort: engine::Sort::Alpha,
            })
        });
        self.screen = super::Screen::Advancements;
        self.capture(false);
    }

    fn hats_close_menu(&mut self) {
        window(|w| w.menu = None);
        if self.screen == super::Screen::Advancements {
            self.screen = super::Screen::Playing;
            self.capture(true);
        }
    }

    fn hats_menu_open(&self) -> bool {
        self.screen == super::Screen::Advancements && window(|w| w.menu.is_some())
    }

    /// HEAD `native_hooks::Game::native_key`: the `key.hats.open_menu` key (H) opens the menu while playing.
    pub(super) fn hats_key(&mut self, key: &str, pressed: bool) {
        if !pressed || self.screen != super::Screen::Playing || self.player.is_none() && self.remote.is_none() {
            return;
        }
        if key == self.native_mod_key(ID, "open_menu", "key.keyboard.h") {
            self.hats_open_menu(Mode::Browse);
        }
    }

    /// The player's collection as the window sees it.
    fn hats_my_data(&self) -> (String, engine::PlayerData) {
        let key = self.hats_own_uuid().map(uuid_text).unwrap_or_default();
        let data = shared(|s| s.save.players.get(&key).cloned().unwrap_or_default());
        (key, data)
    }

    /// The menu's layout on a GUI of `w` x `h`: the grid panel and the details panel.
    fn hats_layout(w: f32, h: f32) -> MenuLayout {
        let panel = (5.0, 5.0, 190.0, (h - 10.0).max(60.0));
        let details = ((w - 5.0 - 160.0).max(200.0), 5.0, 160.0, (h - 10.0).max(60.0));
        let cell = 58.0;
        let grid = (panel.0 + 6.0, panel.1 + 36.0, cell, ((panel.3 - 42.0) / cell).floor().max(1.0) as usize);
        MenuLayout { panel, details, grid }
    }

    /// HEAD cancellable `advancements_screen::Game::draw_advancements`: the menu when it is open.
    pub(super) fn hats_draw_menu(&self, batch: &mut HudBatch) -> bool {
        if !self.hats_menu_open() {
            return false;
        }
        let Some(menu) = window(|w| w.menu.clone()) else {
            return false;
        };
        let reg = self.hats_reg();
        let atlas = &self.gui.atlas;
        let (w, h) = (batch.width, batch.height);
        batch.fill_gradient(atlas, (0.0, 0.0, w, h), 0xC010_1010, 0xD010_1010);
        let l = Self::hats_layout(w, h);
        let (key, data) = self.hats_my_data();
        let _ = key;
        let list = engine::menu_list(reg, &data.inventory, "", menu.sort);
        // The grid panel.
        let (px, py, pw, ph) = l.panel;
        batch.fill(atlas, px, py, pw, ph, 0xC000_0000);
        let title = if menu.mode == Mode::Launch { "Launch which hat?".to_owned() } else { self.translate("hats.gui.title", "Hats") };
        batch.text(atlas, &title, px + 6.0, py + 6.0, 0xFFFF_FFFF, true);
        let count = format!("{} hats", list.len());
        batch.text(atlas, &count, px + 6.0, py + 20.0, 0xFFAA_AAAA, false);
        let sort = engine::Sort::label(menu.sort);
        self.hats_button(batch, (px + pw - 66.0, py + 4.0, 60.0, 14.0), sort, true);
        let (gx, gy, cell, rows) = l.grid;
        let cols = 3;
        let first = menu.scroll * cols;
        let equipped = data.equipped.as_ref().map(|e| e.name.clone());
        for (n, &i) in list.iter().enumerate().skip(first).take(rows * cols) {
            let k = n - first;
            let (cx, cy) = (gx + (k % cols) as f32 * cell, gy + (k / cols) as f32 * cell);
            let part = &data.inventory[i];
            let selected = menu.selected.as_deref() == Some(part.name.as_str());
            let hovered = self.cursor.0 >= cx && self.cursor.0 < cx + cell - 2.0 && self.cursor.1 >= cy && self.cursor.1 < cy + cell - 2.0;
            let bg = if selected { 0x80FF_FFFF } else if hovered { 0x50FF_FFFF } else { 0x40FF_FFFF };
            batch.fill(atlas, cx, cy, cell - 2.0, cell - 2.0, bg);
            if equipped.as_deref() == Some(part.name.as_str()) {
                batch.fill(atlas, cx, cy, cell - 2.0, 2.0, 0xFF55_FF55);
            }
            if let Some(def) = reg.hat_named(&part.name) {
                self.hats_icon(batch, def, cx + 5.0, cy + 3.0, 46.0);
                if part.is_new {
                    batch.text(atlas, "!", cx + 2.0, cy + 2.0, 0xFFFF_FF55, true);
                }
                if part.favourite {
                    batch.text(atlas, "*", cx + cell - 10.0, cy + 2.0, 0xFFFF_AA00, true);
                }
                if part.count > 1 {
                    let c = format!("x{}", part.count);
                    let cw = HudBatch::text_width(atlas, &c);
                    batch.text(atlas, &c, cx + cell - 4.0 - cw, cy + cell - 12.0, 0xFFFF_FFFF, true);
                }
            }
        }
        // The details panel: the selected hat, its accessories, the buttons.
        let (dx, dy, dw, dh) = l.details;
        batch.fill(atlas, dx, dy, dw, dh, 0xC000_0000);
        let selected = menu.selected.as_ref().and_then(|n| data.inventory.iter().find(|p| &p.name == n));
        match selected {
            None => {
                let hint = if list.is_empty() { "No hats yet: kill a mob wearing one." } else { "Pick a hat." };
                for (i, line) in self.gui.atlas.font.split_lines(&Text::literal(hint), dw - 12.0).iter().enumerate() {
                    batch.text_styled(atlas, &line.0, dx + 6.0, dy + 6.0 + 10.0 * i as f32, 0xFFAA_AAAA, false);
                }
            }
            Some(part) => {
                let def = reg.hat_named(&part.name);
                let colour = def.map_or(0xFFFF_FFFF, |d| if d.contributor { 0xFF55_FFFF } else { d.rarity.colour() });
                batch.text(atlas, &part.name, dx + 6.0, dy + 6.0, colour, true);
                if let Some(d) = def {
                    let r = d.rarity.name();
                    let r = format!("{}{}", r[..1].to_uppercase(), &r[1..]);
                    batch.text(atlas, &r, dx + 6.0, dy + 18.0, colour, false);
                    self.hats_icon(batch, d, dx + (dw - 64.0) / 2.0, dy + 32.0, 64.0);
                }
                let mut y = dy + 102.0;
                for (k, acc) in part.accessories.iter().enumerate() {
                    let on = if acc.showing { "[x] " } else { "[ ] " };
                    let hovered = self.cursor.0 >= dx && self.cursor.0 < dx + dw && self.cursor.1 >= y && self.cursor.1 < y + 11.0;
                    let c = if hovered { 0xFFFF_FFA0 } else { 0xFFDD_DDDD };
                    batch.text(atlas, &format!("{on}{}", acc.name), dx + 6.0, y, c, false);
                    y += 11.0;
                    let _ = k;
                }
                let b = Self::hats_buttons(dx, dy, dw, dh);
                if menu.mode == Mode::Launch {
                    self.hats_button(batch, b[0], "Launch", true);
                } else {
                    let is_eq = equipped.as_deref() == Some(part.name.as_str());
                    let label = if is_eq { self.translate("hats.gui.equipped", "Equipped") } else { self.translate("hats.gui.equip", "Equip") };
                    self.hats_button(batch, b[0], &label, !is_eq);
                }
                let fav = if part.favourite { "Unfavourite" } else { "Favourite" };
                self.hats_button(batch, b[1], fav, true);
            }
        }
        let b = Self::hats_buttons(dx, dy, dw, dh);
        self.hats_button(batch, b[2], &self.translate("hats.gui.remove", "Remove"), equipped.is_some() && menu.mode == Mode::Browse);
        self.hats_button(batch, b[3], &self.translate("gui.done", "Done"), true);
        // The hovered hat's name, rarity and count (the grid's tooltip).
        for (n, &i) in list.iter().enumerate().skip(first).take(rows * cols) {
            let k = n - first;
            let (cx, cy) = (gx + (k % cols) as f32 * cell, gy + (k / cols) as f32 * cell);
            if self.cursor.0 >= cx && self.cursor.0 < cx + cell - 2.0 && self.cursor.1 >= cy && self.cursor.1 < cy + cell - 2.0 {
                let part = &data.inventory[i];
                let code = reg.hat_named(&part.name).map_or('f', |d| ['f', 'a', 'b', 'd', '6'][d.rarity.index()]);
                let rarity = reg.hat_named(&part.name).map_or("", |d| d.rarity.name());
                let text = format!("\u{a7}{code}{} \u{a7}7({rarity}, x{})", part.name, part.count);
                self.tooltip(batch, &text, self.cursor.0, self.cursor.1);
            }
        }
        true
    }

    fn hats_buttons(dx: f32, dy: f32, dw: f32, dh: f32) -> [(f32, f32, f32, f32); 4] {
        let bw = dw - 12.0;
        let base = dy + dh - 4.0 * 22.0 - 4.0;
        std::array::from_fn(|i| (dx + 6.0, base + 22.0 * i as f32, bw, 20.0))
    }

    fn hats_button(&self, batch: &mut HudBatch, r: (f32, f32, f32, f32), label: &str, active: bool) {
        let atlas = &self.gui.atlas;
        let hovered = active && self.cursor.0 >= r.0 && self.cursor.0 < r.0 + r.2 && self.cursor.1 >= r.1 && self.cursor.1 < r.1 + r.3;
        let sprite = if !active {
            "gui/sprites/widget/button_disabled"
        } else if hovered {
            "gui/sprites/widget/button_highlighted"
        } else {
            "gui/sprites/widget/button"
        };
        horizon_client::ui::nine_slice(batch, atlas, sprite, r.0, r.1, r.2, r.3, u32::MAX);
        let w = HudBatch::text_width(atlas, label);
        let colour = if active { 0xFFFF_FFFF } else { 0xFFA0_A0A0 };
        batch.text(atlas, label, (r.0 + (r.2 - w) / 2.0).floor(), r.1 + (r.3 - 8.0) / 2.0, colour, true);
    }

    /// HEAD cancellable `advancements_mouse`: clicks on the menu.
    pub(super) fn hats_mouse(&mut self, pressed: bool, size: [u32; 2]) -> bool {
        if !self.hats_menu_open() {
            return false;
        }
        if !pressed {
            return true;
        }
        let batch = HudBatch::new(size);
        let (w, h) = (batch.width, batch.height);
        let l = Self::hats_layout(w, h);
        let (px, py) = self.cursor;
        let inside = |r: (f32, f32, f32, f32)| px >= r.0 && px < r.0 + r.2 && py >= r.1 && py < r.1 + r.3;
        let Some(menu) = window(|w| w.menu.clone()) else {
            return true;
        };
        let reg = self.hats_reg();
        let (key, data) = self.hats_my_data();
        let list = engine::menu_list(reg, &data.inventory, "", menu.sort);
        let (gx, gy, cell, rows) = l.grid;
        // Sort button.
        let (ppx, ppy, ppw, _) = l.panel;
        if inside((ppx + ppw - 66.0, ppy + 4.0, 60.0, 14.0)) {
            self.audio.play_button_click();
            window(|w| {
                if let Some(m) = w.menu.as_mut() {
                    m.sort = m.sort.next();
                }
            });
            return true;
        }
        // A grid cell.
        for (n, &i) in list.iter().enumerate().skip(menu.scroll * 3).take(rows * 3) {
            let k = n - menu.scroll * 3;
            let (cx, cy) = (gx + (k % 3) as f32 * cell, gy + (k / 3) as f32 * cell);
            if inside((cx, cy, cell - 2.0, cell - 2.0)) {
                self.audio.play_button_click();
                let name = data.inventory[i].name.clone();
                window(|w| {
                    if let Some(m) = w.menu.as_mut() {
                        m.selected = Some(name.clone());
                    }
                });
                // The "new" mark goes once seen.
                shared(|s| {
                    if let Some(p) = s.save.player(&key).inventory.iter_mut().find(|p| p.name == name) {
                        p.is_new = false;
                    }
                    s.dirty = true;
                });
                return true;
            }
        }
        let (dx, dy, dw, dh) = l.details;
        let b = Self::hats_buttons(dx, dy, dw, dh);
        let selected = menu.selected.clone();
        if inside(b[3]) {
            self.audio.play_button_click();
            self.hats_close_menu();
            return true;
        }
        if inside(b[2]) && menu.mode == Mode::Browse && data.equipped.is_some() {
            // removeEquipped
            self.audio.play_button_click();
            shared(|s| {
                s.save.player(&key).equipped = None;
                s.dirty = true;
            });
            return true;
        }
        let Some(name) = selected else {
            return true;
        };
        if inside(b[0]) {
            self.audio.play_button_click();
            if menu.mode == Mode::Launch {
                window(|w| w.pending_launch = Some(name));
                self.hats_close_menu();
            } else {
                // equipSelected: the hat with the accessories as shown in the panel.
                shared(|s| {
                    let p = s.save.player(&key);
                    if let Some(owned) = p.inventory.iter().find(|h| h.name == name) {
                        p.equipped = Some(engine::equip_copy(owned));
                    }
                    s.dirty = true;
                });
            }
            return true;
        }
        if inside(b[1]) {
            self.audio.play_button_click();
            shared(|s| {
                if let Some(p) = s.save.player(&key).inventory.iter_mut().find(|h| h.name == name) {
                    p.favourite = !p.favourite;
                }
                s.dirty = true;
            });
            return true;
        }
        // An accessory line: show / hide it (and on the worn hat at once).
        let mut y = dy + 102.0;
        if let Some(part) = data.inventory.iter().find(|p| p.name == name) {
            for acc in &part.accessories {
                if inside((dx, y, dw, 11.0)) {
                    self.audio.play_button_click();
                    let acc_name = acc.name.clone();
                    shared(|s| {
                        let p = s.save.player(&key);
                        if let Some(owned) = p.inventory.iter_mut().find(|h| h.name == name)
                            && let Some(a) = owned.accessories.iter_mut().find(|a| a.name == acc_name)
                        {
                            a.showing = !a.showing;
                            let copy = owned.clone();
                            if p.equipped.as_ref().is_some_and(|e| e.name == name) {
                                p.equipped = Some(engine::equip_copy(&copy));
                            }
                        }
                        s.dirty = true;
                    });
                    return true;
                }
                y += 11.0;
            }
        }
        true
    }

    /// HEAD cancellable `advancements_scroll`: the grid scrolls by rows.
    pub(super) fn hats_scroll(&mut self, y: f32) -> bool {
        if !self.hats_menu_open() {
            return false;
        }
        let (_, data) = self.hats_my_data();
        let rows = data.inventory.len().div_ceil(3);
        window(|w| {
            if let Some(m) = w.menu.as_mut() {
                if y > 0.0 {
                    m.scroll = m.scroll.saturating_sub(1);
                } else if y < 0.0 && m.scroll + 1 < rows {
                    m.scroll += 1;
                }
            }
        });
        true
    }

    /// HEAD cancellable `advancements_drag`: the vanilla tree must not pan under the menu.
    pub(super) fn hats_drag(&self) -> bool {
        self.hats_menu_open()
    }
}

/// The menu's rectangles: (x, y, w, h) panels and the grid (x, y, cell, rows).
struct MenuLayout {
    panel: (f32, f32, f32, f32),
    details: (f32, f32, f32, f32),
    grid: (f32, f32, f32, usize),
}

/// The world as a thrown hat sees it: solid blocks by their collision.
struct Solid<'a, 'g>(&'a Game<'g>);
impl engine::HatWorld for Solid<'_, '_> {
    fn solid(&self, p: [f64; 3]) -> bool {
        let b = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
        let f = self.0.res.facts().get(self.0.world.block(b[0], b[1], b[2]));
        if f.full_collision {
            return true;
        }
        if !f.legacy_solid {
            return false;
        }
        let l = [p[0] - f64::from(b[0]), p[1] - f64::from(b[1]), p[2] - f64::from(b[2])];
        f.outline.iter().any(|o| l[0] >= o[0] && l[0] <= o[3] && l[1] >= o[1] && l[1] <= o[4] && l[2] >= o[2] && l[2] <= o[5])
    }
}
