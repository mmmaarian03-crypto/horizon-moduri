// Carry On inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs): the lists, the pickup checks,
// the slowness, the merge of the placed state, the death spot, the stacking. This part reads and changes the game: the block and its
// block entity, the mob, the player's data (`CarryOnData` is kept in the player's extra data, as the original puts it in the
// player's NBT), the key, the hotbar slot.
//
// Hooks (mixins/carryon/horizon-mixin.toml): use_golem_statue (a right click on a block, hands empty also when sneaking),
// interact_entity (a right click on a mob), tick_blocks (slowness, the locked slot, death), attack + tick_mining (no hitting while
// carrying), draw_selected_name (what is carried, on the screen), native_mods (the key in Options > Mods).
//
// Server and client (Horizon #1514: single player is an integrated server, the window is its guest): the pickup, the placement,
// the slowness, the slot and the death run where the world is, the in-frame host's game or a server, for a guest inside its turn
// (`as_guest`: its body, inventory, keys and data). The guests' tick is `tick_guests`. What the client needs (what is carried, the
// slot) comes over foundry's channel `carryon:sync` (`CarryOnDataManager`'s sync of the original); the client's carry key goes the
// other way on `carryon:key` (the original's key packet), so a key other than Shift works for guests too.
#[allow(unused_imports)]
use super::{Game, Result, Screen, lan::LanHost};
use std::cell::RefCell;
use std::collections::HashMap;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_core::position::BlockPos;
use horizon_world::level::{PlaceContext, PlaceOutcome};
use horizon_world::nbt::Tag;

#[allow(dead_code)]
mod engine {
    include!("carryon_engine.rs");
}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
#[allow(dead_code)]
const MOD_ID: &str = "carryon";

include!("kit_blocks.rs");
include!("kit_notice.rs");
include!("kit_sound.rs");

pub(super) const ID: &str = "carryon";
pub(super) const TITLE: &str = "Carry On 2.1.2.7";
pub(super) const DESCRIPTION: &str = "Carry On: pick up chests, furnaces and other tile entities, and animals, with empty hands and the carry key (Shift); right click to put them down.";
/// `key.carry.desc`: Left Shift by default on Forge (`CarryOnKeybinds`).
const KEY_DEFAULT: &str = "key.keyboard.left.shift";

// ------------------------------------------------------------------------------------------------
// Server <-> client
// ------------------------------------------------------------------------------------------------

/// What the client is told about its player's carry: the type name (`BLOCK`, `ENTITY`, `INVALID`), the block or mob id, the
/// slot that stays selected.
type Sync = (String, String, u32);
/// Server -> client: the player's carry, whenever it changes (and once to every new guest).
const SYNC: super::foundry::Channel<Sync> = super::foundry::Channel::new("carryon:sync");
/// Client -> server: the client's carry key is held or not (sent when it changes).
const KEY: super::foundry::Channel<bool> = super::foundry::Channel::new("carryon:key");

#[derive(Default)]
struct ServerSide {
    /// The last carry each guest's client was told.
    sent: HashMap<u32, Sync>,
    /// The carry key each guest's client last reported.
    keys: HashMap<u32, bool>,
}
#[derive(Default)]
struct ClientSide {
    /// The carry of this window's player, from its server.
    carry: Option<Sync>,
    /// The key state last sent to the server.
    key_sent: Option<bool>,
}
thread_local! {
    // The integrated server runs on its own thread, the window on another: each has its half.
    static SERVER: RefCell<ServerSide> = RefCell::new(ServerSide::default());
    static CLIENT: RefCell<ClientSide> = RefCell::new(ClientSide::default());
}

fn carryon_register() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::foundry::register_server_channel(KEY.id, carryon_key_from_client);
}

/// A guest's client reported its carry key.
fn carryon_key_from_client(_game: &mut Game<'_>, _host: &mut LanHost, conn: u32, body: &[u8]) {
    if let Some(held) = KEY.decode(body) {
        SERVER.with(|s| s.borrow_mut().keys.insert(conn, held));
    }
}

/// The carry kept in a player's compound (`level.player_extra` of the host or of the guest whose turn it is, `saved` of a guest
/// outside its turn).
fn carry_of(extra: &Option<Tag>) -> Option<Carry> {
    let Some(Tag::Compound(extra)) = extra.as_ref() else { return None };
    let Some(Tag::Compound(nbt)) = extra.get(&"CarryOnData".into()) else { return None };
    let ty = engine::CarryType::from_name(&horizon_client::save::text(cg(nbt, "type")).unwrap_or_default());
    (ty != engine::CarryType::Invalid).then(|| Carry { ty, nbt: nbt.clone() })
}

/// What the client is told about a carry.
fn sync_of(carry: Option<&Carry>) -> Sync {
    match carry {
        Some(c) => {
            let id = match c.ty {
                engine::CarryType::Block => c.block_name().unwrap_or_default(),
                _ => horizon_client::save::text(cg(&c.nbt, "entity").and_then(|e| e.get("id"))).unwrap_or_default(),
            };
            (c.ty.name().to_owned(), id, c.selected() as u32)
        }
        None => ("INVALID".to_owned(), String::new(), 0),
    }
}

// ------------------------------------------------------------------------------------------------
// CarryOnData
// ------------------------------------------------------------------------------------------------

/// `CarryOnData`: the compound the original keeps under `CarryOnData` in the player's NBT.
struct Carry {
    ty: engine::CarryType,
    nbt: horizon_world::nbt::Compound,
}
impl Carry {
    fn block_name(&self) -> Option<String> {
        horizon_client::save::text(cg(&self.nbt, "block").and_then(|b| b.get("Name")))
    }
    fn selected(&self) -> usize {
        horizon_client::save::number(cg(&self.nbt, "selected")).unwrap_or(0).clamp(0, 8) as usize
    }
    /// `carry.getNbt().toString()`: the length drives the slowness of a block.
    fn nbt_len(&self) -> usize {
        horizon_client_core::snbt::to_snbt(&Tag::Compound(self.nbt.clone())).len()
    }
}

/// A compound's entry by name (the keys are Java strings).
fn cg<'a>(c: &'a horizon_world::nbt::Compound, k: &str) -> Option<&'a Tag> {
    c.get(&horizon_world::nbt::JavaString::from(k))
}

fn s(v: &str) -> Tag {
    Tag::String(v.into())
}

/// `ServerLevel` name of a mob type for the lists: `minecraft:pig`.
fn kind_id(id: &str) -> String {
    if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") }
}

impl Game<'_> {
    // ---- the data -----------------------------------------------------------------------------

    fn co_carry(&self) -> Option<Carry> {
        carry_of(&self.level.player_extra)
    }
    fn co_set_carry(&mut self, carry: Option<Carry>) {
        let extra = self.level.player_extra.get_or_insert_with(|| Tag::Compound(Default::default()));
        let Tag::Compound(map) = extra else { return };
        match carry {
            Some(mut c) => {
                c.nbt.insert("type".into(), s(c.ty.name()));
                c.nbt.insert("keyPressed".into(), Tag::Byte(0));
                c.nbt.insert("selected".into(), Tag::Int(self.selected as i32));
                c.nbt.insert("tick".into(), Tag::Int(self.world.game_time as i32));
                map.insert("CarryOnData".into(), Tag::Compound(c.nbt));
            }
            None => {
                // `carry.clear()`: INVALID, remembering the tick so a pickup and a placement never share one.
                let mut nbt = horizon_world::nbt::Compound::new();
                nbt.insert("type".into(), s("INVALID"));
                nbt.insert("tick".into(), Tag::Int(self.world.game_time as i32));
                map.insert("CarryOnData".into(), Tag::Compound(nbt));
            }
        }
    }
    /// The tick of the last pickup or placement (kept after `clear`).
    fn co_last_tick(&self) -> Option<i64> {
        let Some(Tag::Compound(extra)) = self.level.player_extra.as_ref() else { return None };
        horizon_client::save::number(extra.get(&"CarryOnData".into()).and_then(|c| c.get("tick")))
    }

    /// The carry key is held (`CarryOnKeybinds.carryKey`, Left Shift unless the player changed it). In a guest's turn the keys
    /// are the guest's (Shift while it sneaks); its client also reports its own carry key (`carryon:key`), whatever it is bound to.
    fn co_key_held(&self) -> bool {
        let name = self.native_mod_key(ID, "carry", KEY_DEFAULT);
        let here = self.keys.iter().any(|k| horizon_client::keybinds::key_name(*k) == Some(name.as_str()));
        let reported = self.lan_acting.is_some_and(|conn| SERVER.with(|s| s.borrow().keys.get(&conn).copied().unwrap_or(false)));
        here || reported
    }

    /// Whether the registered name of a state is in a tag (the lists use `#` tags; Horizon has none for `forge:` names).
    fn co_in_block_tag(&self, tag: &str, name: &str) -> bool {
        kit_blocks::block_in_tag(self, tag, name)
    }

    /// What `canCarryGeneral` needs.
    fn co_carrier(&self, distance: f64) -> engine::Carrier {
        let hands_empty = self.stack_in(self.selected).is_none() && self.offhand.is_none();
        engine::Carrier {
            hands_empty,
            distance,
            carrying: self.co_carry().is_some(),
            key_pressed: self.co_key_held(),
            same_tick: self.co_last_tick() == Some(self.world.game_time as i32 as i64),
            game_mode_allows: self.may_build(),
            creative: self.creative,
        }
    }

    fn co_player_pos(&self) -> Option<[f64; 3]> {
        self.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z])
    }

    // ---- blocks -------------------------------------------------------------------------------

    /// HEAD on `use_golem_statue` (a right click on a block, before the block's own use): the carried object goes down, or an
    /// empty-handed player with the carry key picks the block up. True when the click was ours. A server action: it runs on the
    /// in-frame host's game and on a server inside the guest's turn; a server's client only sends the click.
    pub(super) fn carryon_use_block(&mut self, pos: [i32; 3]) -> bool {
        if self.remote.is_some() {
            return false;
        }
        carryon_register();
        match self.co_carry() {
            Some(c) => {
                let face = self.pick().map(|h| h.face).unwrap_or([0, 1, 0]);
                match c.ty {
                    engine::CarryType::Block => {
                        self.co_place_block(&c, pos, face);
                    }
                    engine::CarryType::Entity => {
                        self.co_place_entity(&c, pos, face);
                    }
                    _ => {}
                }
                // The original cancels the click whatever came of it.
                true
            }
            None => self.co_pick_block(pos),
        }
    }

    /// `PickupHandler.tryPickUpBlock`.
    fn co_pick_block(&mut self, pos: [i32; 3]) -> bool {
        let Some(p) = self.co_player_pos() else { return false };
        let center = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
        let distance = (0..3).map(|a| (p[a] - center[a]).powi(2)).sum::<f64>().sqrt();
        if engine::can_carry_general(&self.co_carrier(distance)).is_err() {
            return false;
        }
        let key = (pos[0], pos[1], pos[2]);
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let (name, hardness, sound_hit) = {
            let facts = self.res.facts().get(state);
            (facts.name.to_string(), facts.hardness, facts.sounds.hit.to_string())
        };
        let Some(entity) = self.world.block_entities.get(&key) else {
            // Without a block entity only `pickupAllBlocks` would allow it (off); a block entity Horizon does not model
            // is not touched: it would lose its data.
            return false;
        };
        let tile = horizon_client::save::encode_block_entity(key, entity, &name);
        let locked = horizon_client::save::text(tile.get("Lock")).is_some_and(|l| !l.is_empty());
        let facts = engine::BlockFacts { name: name.clone(), has_block_entity: true, unbreakable: hardness < 0.0, locked };
        let in_tag = |t: &str| self.co_in_block_tag(t, &name);
        if engine::can_pick_block(&facts, self.creative, &in_tag).is_err() {
            return false;
        }
        // CarryOnData.setBlock: the state (not waterlogged) and the block entity with its id.
        let mut props = horizon_world::nbt::Compound::new();
        if let Some(st) = self.pack.assets.registry.state(state) {
            for (k, v) in &st.properties {
                let v = if k == "waterlogged" { "false" } else { v.as_str() };
                props.insert(k.as_str().into(), s(v));
            }
        }
        let mut block = horizon_world::nbt::Compound::new();
        block.insert("Name".into(), s(&name));
        if !props.is_empty() {
            block.insert("Properties".into(), Tag::Compound(props));
        }
        let mut nbt = horizon_world::nbt::Compound::new();
        nbt.insert("block".into(), Tag::Compound(block));
        nbt.insert("tile".into(), tile);
        // level.removeBlockEntity(pos); level.removeBlock(pos, false): the contents go with the block, nothing spills.
        self.world.block_entities.remove(&key);
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        self.with_level(|b, l, t| b.remove_block(l, t, at, false));
        let _ = self.process_level_events();
        self.co_set_carry(Some(Carry { ty: engine::CarryType::Block, nbt }));
        kit_sound::at_block(self, &sound_hit, pos, 1.0, 0.5);
        self.swing_arm();
        true
    }

    /// `PlacementHandler.tryPlaceBlock`.
    fn co_place_block(&mut self, carry: &Carry, clicked: [i32; 3], face: [i32; 3]) -> bool {
        if self.co_last_tick() == Some(self.world.game_time as i32 as i64) {
            return false;
        }
        let Some(name) = carry.block_name() else {
            self.co_set_carry(None);
            return false;
        };
        let replaceable = |g: &Game<'_>, p: [i32; 3]| {
            let id = g.world.block(p[0], p[1], p[2]);
            g.res.facts().is_air(id) || g.res.facts().get(id).replaceable
        };
        let pos = engine::placement_pos(clicked, face, replaceable(self, clicked));
        let Some(registry) = Some(&self.pack.assets.registry) else { return false };
        // The carried state.
        let carried_props: Vec<(String, String)> = match cg(&carry.nbt, "block").and_then(|b| b.get("Properties")) {
            Some(Tag::Compound(m)) => m.iter().filter_map(|(k, v)| Some((k.to_string_lossy(), horizon_client::save::text(Some(v))?))).collect(),
            _ => Vec::new(),
        };
        let Some(default) = registry.default_state(&name) else { return false };
        let item = self.res.facts().get(default).item.to_string();
        let Some(p) = self.co_player_pos() else { return false };
        let can = self.world.loaded(pos[0], pos[2]) && replaceable(self, pos);
        if !can || item.is_empty() || item == "minecraft:air" {
            kit_sound::at(self, "minecraft:block.lava.pop", p, 0.5, 0.5);
            return false;
        }
        // The state a block of that kind gets when placed there: the game's own item placement does it.
        let ctx = PlaceContext {
            clicked_pos: BlockPos::new(pos[0], pos[1], pos[2]),
            face: direction_of(face),
            hit: horizon_core::geometry::Vec3::new(p[0], p[1], p[2]),
            inside: false,
            yaw: self.yaw,
            pitch: self.pitch,
            sneaking: kit_blocks::sneaking(self),
            item,
            can_use_game_master_blocks: false,
        };
        let outcome = self.with_level(|b, l, t| b.place_block(l, t, &ctx));
        let _ = self.process_level_events();
        let PlaceOutcome::Placed { state: placed, .. } = outcome else {
            kit_sound::at(self, "minecraft:block.lava.pop", p, 0.5, 0.5);
            return false;
        };
        // The carried state with the direction, axis and exception properties of the placed one.
        let placed_name = self.res.facts().get(placed).name.to_string();
        let mut final_state = placed;
        if placed_name == name {
            let placed_props: Vec<(String, String)> =
                self.pack.assets.registry.state(placed).map(|st| st.properties.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()).unwrap_or_default();
            let mut carried = carried_props;
            // Properties the carried state lacks (a newer block) take the placed ones.
            for (k, v) in &placed_props {
                if !carried.iter().any(|(n, _)| n == k) {
                    carried.push((k.clone(), v.clone()));
                }
            }
            let merged = engine::merge_placement_state(&carried, &placed_props, &engine::property_exceptions());
            let pairs: Vec<(&str, &str)> = merged.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
            if let Some(st) = kit_blocks::with_props(self, default, &pairs) {
                final_state = st;
            }
        }
        if final_state != placed {
            let at = BlockPos::new(pos[0], pos[1], pos[2]);
            self.with_level(|b, l, t| b.set_block(l, t, at, final_state, 3));
            let _ = self.process_level_events();
        }
        // The block entity: the carried one, moved to its new place.
        if let Some(Tag::Compound(tile)) = cg(&carry.nbt, "tile") {
            let mut tile = tile.clone();
            tile.insert("x".into(), Tag::Int(pos[0]));
            tile.insert("y".into(), Tag::Int(pos[1]));
            tile.insert("z".into(), Tag::Int(pos[2]));
            if let Some((_, entity)) = horizon_client::save::decode_block_entity(&Tag::Compound(tile)) {
                self.world.block_entities.insert((pos[0], pos[1], pos[2]), entity);
            }
        }
        self.co_set_carry(None);
        let place_sound = self.res.facts().get(final_state).sounds.place.to_string();
        kit_sound::at_block(self, &place_sound, pos, 1.0, 0.5);
        self.swing_arm();
        true
    }

    // ---- entities -----------------------------------------------------------------------------

    /// HEAD on `interact_entity` (a right click with the acting hand on what is aimed at): a mob is picked up, or the carried one
    /// is stacked on the mob clicked. True when the click was ours. A server action, as `carryon_use_block`.
    pub(super) fn carryon_interact(&mut self) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let Some((index, distance)) = self.mob_hit() else { return false };
        // A block nearer than the mob takes the click (interact_mob).
        if let Some(hit) = self.pick()
            && let Some(eye) = self.eye_position()
        {
            let d = (0..3).map(|a| (hit.point[a] - eye[a]).powi(2)).sum::<f64>().sqrt();
            if d < distance {
                return false;
            }
        }
        match self.co_carry() {
            None => self.co_pick_mob(index),
            Some(c) if c.ty == engine::CarryType::Entity => {
                self.co_stack_mob(&c, index);
                false
            }
            Some(_) => false,
        }
    }

    fn co_mob_area(&self, index: usize) -> (f64, f64) {
        let size = self.mobs[index].bounds().size();
        (size.x.max(size.z), size.y)
    }

    /// `PickupHandler.tryPickupEntity`.
    fn co_pick_mob(&mut self, index: usize) -> bool {
        let Some(p) = self.co_player_pos() else { return false };
        let m = &self.mobs[index];
        let c = m.center();
        let distance = (0..3).map(|a| (p[a] - [m.position.x, m.position.y, m.position.z][a]).powi(2)).sum::<f64>().sqrt();
        let _ = c;
        if engine::can_carry_general(&self.co_carrier(distance)).is_err() {
            return false;
        }
        let kind = kind_id(&m.kind.id);
        let (width, height) = self.co_mob_area(index);
        let me = self.acting_uuid();
        let facts = engine::EntityFacts {
            kind: kind.clone(),
            is_monster: m.kind.category.eq_ignore_ascii_case("monster"),
            width,
            height,
            invulnerable_time: m.invulnerable != 0,
            removed: m.removed || !m.alive(),
            owned_by_other: m.extra.owner.is_some_and(|o| o != me),
            is_baby: m.extra.age < 0,
            is_player: false,
            other_player_creative: false,
        };
        let in_tag = |_: &str| false;
        if engine::can_pick_entity(&facts, self.creative, &in_tag).is_err() {
            return false;
        }
        let id = m.id;
        // entity.ejectPassengers(); entity.stopRiding(); dropLeash.
        for other in &mut self.mobs {
            if other.extra.mount == Some(id) {
                other.extra.mount = None;
                other.extra.passenger = false;
            }
        }
        if let Some(vehicle) = self.mobs[index].extra.mount.take() {
            self.mobs[index].extra.passenger = false;
            if let Some(v) = self.mobs.iter_mut().find(|v| v.id == vehicle) {
                v.extra.rider = None;
            }
        }
        self.mobs[index].extra.rider = None;
        self.break_leash(id, true, false);
        let tag = self.mob_tree_nbt(index);
        let mut nbt = horizon_world::nbt::Compound::new();
        nbt.insert("entity".into(), tag);
        // `entity.remove(UNLOADED_WITH_PLAYER)`.
        self.mobs[index].removed = true;
        self.co_set_carry(Some(Carry { ty: engine::CarryType::Entity, nbt }));
        let at = [p[0], p[1] - 0.2, p[2]];
        kit_sound::at(self, "minecraft:item.armor.equip_generic", at, 1.0, 0.5);
        self.swing_arm();
        true
    }

    /// The carried mob as a game mob, put at `at` (feet). `mount`: the mob it rides.
    fn co_spawn_carried(&mut self, carry: &Carry, at: [f64; 3], mount: Option<u32>) -> Option<u32> {
        let Some(Tag::Compound(entity)) = cg(&carry.nbt, "entity") else { return None };
        let mut entity = entity.clone();
        entity.insert(
            "Pos".into(),
            Tag::List(at.iter().map(|v| Tag::Double(v.to_bits())).collect()),
        );
        entity.insert("Motion".into(), Tag::List(vec![Tag::Double(0f64.to_bits()); 3]));
        entity.insert("OnGround".into(), Tag::Byte(0));
        self.load_mob(Tag::Compound(entity), mount)
    }

    /// `PlacementHandler.tryPlaceEntity`.
    fn co_place_entity(&mut self, carry: &Carry, clicked: [i32; 3], face: [i32; 3]) -> bool {
        if self.co_last_tick() == Some(self.world.game_time as i32 as i64) {
            return false;
        }
        let Some(p) = self.co_player_pos() else { return false };
        let replaceable = |g: &Game<'_>, q: [i32; 3]| {
            let id = g.world.block(q[0], q[1], q[2]);
            g.res.facts().is_air(id) || g.res.facts().get(id).replaceable
        };
        let mut pos = clicked;
        if !replaceable(self, pos) {
            pos = engine::placement_pos(clicked, face, false);
        }
        if !replaceable(self, pos) {
            kit_sound::at(self, "minecraft:block.lava.pop", p, 0.5, 0.5);
            return false;
        }
        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5];
        if self.co_spawn_carried(carry, at, None).is_none() {
            return false;
        }
        self.co_set_carry(None);
        self.swing_arm();
        true
    }

    /// `PlacementHandler.tryStackEntity`: the carried mob rides the top of the pile the clicked mob is in.
    fn co_stack_mob(&mut self, carry: &Carry, clicked: usize) {
        let Some(p) = self.co_player_pos() else { return };
        // The root of the pile, its passengers and its top.
        let mut root = clicked;
        for _ in 0..16 {
            let Some(v) = self.mobs[root].extra.mount else { break };
            match self.mobs.iter().position(|m| m.id == v) {
                Some(i) => root = i,
                None => break,
            }
        }
        let mut top = root;
        let mut passengers = 0usize;
        for _ in 0..16 {
            let Some(r) = self.mobs[top].extra.rider else { break };
            match self.mobs.iter().position(|m| m.id == r) {
                Some(i) => {
                    top = i;
                    passengers += 1;
                }
                None => break,
            }
        }
        // The carried mob's size: from its data (the mob type's box).
        let held_kind = horizon_client::save::text(cg(&carry.nbt, "entity").and_then(|e| e.get("id"))).unwrap_or_default();
        let held_area = self.res.mob_types().get(held_kind.trim_start_matches("minecraft:")).map_or(0.0, |t| t.width * t.height);
        let (tw, th) = self.co_mob_area(top);
        let c = &self.mobs[clicked];
        let (cx, cz) = (c.position.x.floor() as i32, c.position.z.floor() as i32);
        let (px, pz) = (p[0].floor() as i32, p[2].floor() as i32);
        let (cy, py) = (c.position.y.floor() as i32, p[1].floor() as i32);
        let facts = engine::StackFacts {
            passengers,
            top_is_held: false,
            top_kind: kind_id(&self.mobs[top].kind.id),
            top_area: tw * th,
            held_area,
            distance_sq: f64::from((cx - px).pow(2) + (cy - py).pow(2) + (cz - pz).pow(2)),
        };
        let in_tag = |_: &str| false;
        match engine::stack_decision(&facts, &in_tag) {
            engine::Stack::No => {}
            engine::Stack::Refused => kit_sound::at(self, "minecraft:block.lava.pop", p, 0.5, 0.5),
            engine::Stack::Stacked { .. } => {
                let top_id = self.mobs[top].id;
                let at = [self.mobs[clicked].position.x, self.mobs[clicked].position.y, self.mobs[clicked].position.z];
                if let Some(new_id) = self.co_spawn_carried(carry, at, Some(top_id))
                    && let Some(t) = self.mobs.iter_mut().find(|m| m.id == top_id)
                {
                    t.extra.rider = Some(new_id);
                    self.co_set_carry(None);
                    self.swing_arm();
                    kit_sound::at(self, "minecraft:entity.horse.saddle", p, 0.5, 1.5);
                }
            }
        }
    }

    // ---- tick ---------------------------------------------------------------------------------

    /// HEAD on `tick_blocks`: `CarryOnCommon.onCarryTick` (the slowness, the slot kept) and the death of the in-frame host. A
    /// server has no player of its own (`self.player` is `None` on the integrated server): its guests tick in `tick_guests`.
    pub(super) fn carryon_tick(&mut self) {
        if self.remote.is_some() || self.lan_acting.is_some() || self.player.is_none() {
            return;
        }
        let Some(carry) = self.co_carry() else { return };
        if self.player.as_ref().is_some_and(|p| p.health.dead_or_dying()) {
            // `placeCarriedOnDeath`: kept with keepInventory, else put down where the player died.
            if !self.ui.rules.bool("keep_inventory")
                && let Some(p) = self.co_player_pos()
            {
                self.co_place_carried(&carry, p);
            }
            return;
        }
        self.co_carrier_tick(&carry);
    }

    /// HEAD on `tick_guests` (the guests' tick on the server; single player's window is a guest): each guest's
    /// `onCarryTick` as the guest, its death, and what its client is told.
    pub(super) fn carryon_guests_tick(&mut self, host: &mut LanHost) {
        if self.remote.is_some() {
            return;
        }
        carryon_register();
        let all: Vec<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
        SERVER.with(|s| {
            let mut s = s.borrow_mut();
            s.sent.retain(|c, _| all.contains(c));
            s.keys.retain(|c, _| all.contains(c));
        });
        for i in 0..host.hub.guests.len() {
            let g = &host.hub.guests[i];
            if !(host.hub.here(g) && g.loaded) {
                continue;
            }
            let conn = g.conn;
            if let Some(carry) = carry_of(&g.saved) {
                if g.dead {
                    // `placeCarriedOnDeath` where the guest died; its data is put in place for the call.
                    if !self.ui.rules.bool("keep_inventory") {
                        let at = g.position;
                        std::mem::swap(&mut self.level.player_extra, &mut host.hub.guests[i].saved);
                        self.co_place_carried(&carry, at);
                        std::mem::swap(&mut self.level.player_extra, &mut host.hub.guests[i].saved);
                    }
                } else if g.placed {
                    let before = g.selected;
                    self.as_guest(host, conn, |game| game.co_carrier_tick(&carry));
                    // `ServerPlayer.onEffectAdded` tells the client at once: the slowness lasts one tick and the guest's
                    // body ticks it away later in this pass, before the hub's own sync would see it.
                    host.hub.sync_all_effects();
                    // The slot the carry keeps, on the guest's client too (`ClientboundSetCarriedItemPacket`).
                    let g = &host.hub.guests[i];
                    if g.selected != before {
                        let slot = g.selected as i32;
                        host.hub.send_to(
                            conn,
                            &horizon_protocol::packets::play::Clientbound::SetHeldSlot(horizon_protocol::packets::play::SetHeldSlot { slot }),
                        );
                    }
                }
            }
            if !super::foundry::guest_has_foundry(conn) {
                continue;
            }
            let now = sync_of(carry_of(&host.hub.guests[i].saved).as_ref());
            let changed = SERVER.with(|s| s.borrow().sent.get(&conn) != Some(&now));
            if changed {
                super::foundry::send_to_guest(host, conn, SYNC.id, &SYNC.encode(&now));
                SERVER.with(|s| s.borrow_mut().sent.insert(conn, now));
            }
        }
    }

    /// HEAD on `native_tick` (every `Game`, the window included): a server's client keeps what its player carries (for the
    /// screen, the slot and the no-hitting), and tells the server when its carry key goes down or up.
    pub(super) fn carryon_client_tick(&mut self) {
        if self.remote.is_none() {
            return;
        }
        for body in super::foundry::client_take(SYNC.id) {
            if let Some(sync) = SYNC.decode(&body) {
                let carrying = engine::CarryType::from_name(&sync.0) != engine::CarryType::Invalid;
                CLIENT.with(|c| c.borrow_mut().carry = carrying.then_some(sync));
            }
        }
        if !super::foundry::server_has_foundry() {
            CLIENT.with(|c| *c.borrow_mut() = ClientSide::default());
            return;
        }
        let name = self.native_mod_key(ID, "carry", KEY_DEFAULT);
        let held = self.keys.iter().any(|k| horizon_client::keybinds::key_name(*k) == Some(name.as_str()));
        if CLIENT.with(|c| c.borrow().key_sent != Some(held)) && super::foundry::send_to_server(self, KEY.id, &KEY.encode(&held)) {
            CLIENT.with(|c| c.borrow_mut().key_sent = Some(held));
        }
        // The slot of the carry stays selected here too (`CarryOnCommonClient.onCarryClientTick`).
        if let Some(slot) = CLIENT.with(|c| c.borrow().carry.as_ref().map(|s| s.2 as usize)) {
            self.selected = slot.min(8);
        }
    }

    /// `onCarryTick` of the player in place (the in-frame host, or a guest in its turn): no digging, the slowness, the slot.
    fn co_carrier_tick(&mut self, carry: &Carry) {
        self.mining = None;
        if engine::slows(self.creative) {
            let load = match carry.ty {
                engine::CarryType::Player => engine::Load::Player,
                engine::CarryType::Entity => {
                    let kind = horizon_client::save::text(cg(&carry.nbt, "entity").and_then(|e| e.get("id"))).unwrap_or_default();
                    let t = self.res.mob_types().get(kind.trim_start_matches("minecraft:"));
                    engine::Load::Entity { area: t.map_or(1.0, |t| t.width * t.height) }
                }
                _ => engine::Load::Block { nbt_len: carry.nbt_len() },
            };
            let amp = engine::slowness_amplifier(load);
            let mut effect = horizon_client::effects::EffectInstance::new("slowness", 1, amp);
            effect.visible = false;
            self.add_player_effect(effect);
        }
        // The slot of the carry stays selected.
        self.selected = carry.selected();
    }

    /// `PlacementHandler.placeCarried`: where the player is (`p`, feet), in the nearest free spot for a block.
    fn co_place_carried(&mut self, carry: &Carry, p: [f64; 3]) {
        match carry.ty {
            engine::CarryType::Entity => {
                self.co_spawn_carried(carry, p, None);
            }
            engine::CarryType::Block => {
                let Some(name) = carry.block_name() else {
                    self.co_set_carry(None);
                    return;
                };
                let here = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
                let Some(default) = self.pack.assets.registry.default_state(&name) else {
                    self.co_set_carry(None);
                    return;
                };
                let props: Vec<(String, String)> = match cg(&carry.nbt, "block").and_then(|b| b.get("Properties")) {
                    Some(Tag::Compound(m)) => m.iter().filter_map(|(k, v)| Some((k.to_string_lossy(), horizon_client::save::text(Some(v))?))).collect(),
                    _ => Vec::new(),
                };
                let pairs: Vec<(&str, &str)> = props.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                let state = kit_blocks::with_props(self, default, &pairs).unwrap_or(default);
                let mut spot = here;
                for candidate in engine::death_candidates(here, p) {
                    if !self.world.loaded(candidate[0], candidate[2]) {
                        continue;
                    }
                    let id = self.world.block(candidate[0], candidate[1], candidate[2]);
                    let free = self.res.facts().is_air(id) || self.res.facts().get(id).replaceable;
                    let at = BlockPos::new(candidate[0], candidate[1], candidate[2]);
                    if free && self.with_level(|b, l, t| b.can_survive(l, t, at, state)) {
                        spot = candidate;
                        break;
                    }
                }
                let at = BlockPos::new(spot[0], spot[1], spot[2]);
                self.with_level(|b, l, t| b.set_block(l, t, at, state, 3));
                let _ = self.process_level_events();
                if let Some(Tag::Compound(tile)) = cg(&carry.nbt, "tile") {
                    let mut tile = tile.clone();
                    tile.insert("x".into(), Tag::Int(spot[0]));
                    tile.insert("y".into(), Tag::Int(spot[1]));
                    tile.insert("z".into(), Tag::Int(spot[2]));
                    if let Some((_, entity)) = horizon_client::save::decode_block_entity(&Tag::Compound(tile)) {
                        self.world.block_entities.insert((spot[0], spot[1], spot[2]), entity);
                    }
                }
            }
            _ => {}
        }
        self.co_set_carry(None);
    }

    /// True while carrying and not allowed to hit (`hitWhileCarrying`): `attack` and `tick_mining` stop.
    /// Runs on the in-frame host, on a server in a guest's turn (the guest's carry), and on a server's client (what its server
    /// said it carries), so the client does not start digging either.
    pub(super) fn carryon_no_hit(&self) -> bool {
        let carrying = if self.remote.is_some() { CLIENT.with(|c| c.borrow().carry.is_some()) } else { self.co_carry().is_some() };
        !engine::may_hit(carrying)
    }

    // ---- HUD ----------------------------------------------------------------------------------

    /// HEAD on `draw_selected_name`: what is carried, at the top of the screen (the original draws the object in front of the player).
    /// A server's client draws what its server said (`carryon:sync`).
    pub(super) fn carryon_hud(&self, batch: &mut HudBatch) {
        if self.screen != Screen::Playing || self.ui.hide_gui {
            return;
        }
        let (ty, id) = if self.remote.is_some() {
            let Some((ty, id, _)) = CLIENT.with(|c| c.borrow().carry.clone()) else { return };
            (engine::CarryType::from_name(&ty), id)
        } else {
            let Some(c) = self.co_carry() else { return };
            let (_, id, _) = sync_of(Some(&c));
            (c.ty, id)
        };
        let atlas = &self.gui.atlas;
        let label = match ty {
            engine::CarryType::Block => {
                let name = id;
                let (ns, path) = name.split_once(':').unwrap_or(("minecraft", name.as_str()));
                let text = horizon_client::lang::get_or(&format!("block.{ns}.{path}"), path);
                if let Some(default) = self.pack.assets.registry.default_state(&name) {
                    let item = self.res.facts().get(default).item.to_string();
                    if !item.is_empty() {
                        self.draw_item(batch, &ItemStack::new(&item, 1), (batch.width / 2.0 - 8.0).floor(), 6.0);
                    }
                }
                text
            }
            engine::CarryType::Entity => {
                let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id.as_str()));
                horizon_client::lang::get_or(&format!("entity.{ns}.{path}"), path)
            }
            _ => String::new(),
        };
        let w = HudBatch::text_width(atlas, &label);
        batch.text(atlas, &label, (batch.width / 2.0 - w / 2.0).floor(), 26.0, rgba(0xFF, 0xFF, 0xFF, 255), true);
    }
}

/// `Direction` from the face normal.
fn direction_of(face: [i32; 3]) -> horizon_core::geometry::Direction {
    use horizon_core::geometry::Direction as D;
    match face {
        [0, 1, 0] => D::Up,
        [0, -1, 0] => D::Down,
        [0, 0, -1] => D::North,
        [0, 0, 1] => D::South,
        [-1, 0, 0] => D::West,
        _ => D::East,
    }
}
