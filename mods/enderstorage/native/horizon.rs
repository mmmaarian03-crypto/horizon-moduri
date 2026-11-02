// EnderStorage inside Horizon's game (nightly 142388e1c). The rules are in `horizon_client::enderstorage`
// (src/lib.rs + native/library.rs, mixins/enderstorage/shared.rs); this part is the game's side.
//
// What the original keeps in `EnderStorageManager` (one inventory and one tank per frequency, in the
// world folder) is `Store`, written to `data/enderstorage/store.nbt` beside the saves. Each Ender
// Chest block also has Horizon's own `BlockEntity::Chest(Vec<..>)` as a working copy of its
// frequency's inventory, so hoppers, comparators and the vanilla chest menu work on it
// (the ironchest pattern); `enderstorage_tick` keeps every copy and the inventory equal
// (`reconcile`). The Ender Pouch opens its frequency's inventory on a placeholder container under a
// vehicle-style key, as a chest minecart does. The Ender Tank has no block entity: its tank is in
// `Store`. Through forja (`crate::game::forja`, a native mod that must be enabled with this one) the
// tank is the fluid handler of its block on every side (`TileEnderTank`'s FLUID_HANDLER), so Pipez
// fluid pipes and forja machines fill and drain the frequency's tank.
use super::{Game, Screen};
use horizon_client::{containers::BlockEntity, enderstorage as lib, items::ItemStack};
use horizon_core::position::BlockPos;
use horizon_core::random::RandomSource;
use horizon_world::level::flags as level_flags;
use horizon_world::nbt::{JavaString, Limits, NamedTag, Tag, read_named, write_named};
use horizon_world::registry::BlockStateId;
use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;
use winit::keyboard::KeyCode;

// The module that the tests run; the game uses the same rules through `horizon_client::enderstorage`.
#[allow(dead_code, unused_imports)]
mod engine {
    include!("enderstorage_engine.rs");
}

/// The placeholder container of the open pouch (the vehicles' key: no reach check, no block).
const POUCH_KEY: (i32, i32, i32) = (i32::MAX - 15, super::minecarts::CART_KEY_Y, 0x4553_5031);
/// The sprite the chest item's model asks for (`{"type": "minecraft:chest", "texture": "enderstorage:ender_chest"}`).
const ITEM_SPRITE: &str = "entity/chest/enderstorage:ender_chest";

static STORE: Mutex<Option<lib::Store<ItemStack>>> = Mutex::new(None);
/// The copies that `reconcile` has seen (dimension, position; the pouch is `("pouch", [0; 3])`).
static SEEN: Mutex<Option<HashSet<(String, [i32; 3])>>> = Mutex::new(None);
/// The frequency of the open pouch.
static POUCH: Mutex<Option<lib::Frequency>> = Mutex::new(None);

fn with_store<R>(f: impl FnOnce(&mut lib::Store<ItemStack>) -> R) -> R {
    let mut guard = STORE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Default::default))
}
fn with_seen<R>(f: impl FnOnce(&mut HashSet<(String, [i32; 3])>) -> R) -> R {
    let mut guard = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashSet::new))
}
fn key_of(p: [i32; 3]) -> (i32, i32, i32) {
    (p[0], p[1], p[2])
}
fn pouch_seen_key() -> (String, [i32; 3]) {
    ("pouch".to_owned(), [0; 3])
}

// ---- the world file ----

fn kind_text(kind: lib::Kind) -> &'static str {
    match kind {
        lib::Kind::Chest => "chest",
        lib::Kind::Tank => "tank",
    }
}
fn store_to_tag(s: &lib::Store<ItemStack>) -> Tag {
    let items = s
        .items
        .iter()
        .map(|(key, st)| {
            Tag::compound([
                ("key", Tag::String(key.as_str().into())),
                ("size", Tag::Int(st.size as i32)),
                ("Items", horizon_client::save::encode_items(&st.items)),
            ])
        })
        .collect();
    let liquids = s
        .liquids
        .iter()
        .filter(|(_, l)| l.fluid.is_some() && l.amount > 0)
        .map(|(key, l)| {
            Tag::compound([
                ("key", Tag::String(key.as_str().into())),
                ("fluid", Tag::String(l.fluid.clone().unwrap_or_default().as_str().into())),
                ("amount", Tag::Int(l.amount as i32)),
            ])
        })
        .collect();
    let blocks = s
        .blocks
        .iter()
        .map(|((dim, p), (kind, freq))| {
            Tag::compound([
                ("dim", Tag::String(dim.as_str().into())),
                ("x", Tag::Int(p[0])),
                ("y", Tag::Int(p[1])),
                ("z", Tag::Int(p[2])),
                ("kind", Tag::String(kind_text(*kind).into())),
                ("Frequency", lib::frequency_tag(freq)),
            ])
        })
        .collect();
    Tag::compound([
        ("items", Tag::List(items)),
        ("liquids", Tag::List(liquids)),
        ("blocks", Tag::List(blocks)),
    ])
}
fn store_from_tag(tag: &Tag) -> lib::Store<ItemStack> {
    use horizon_client::save::{number, text};
    let mut s = lib::Store::<ItemStack>::default();
    let list = |k: &str| match tag.get(k) {
        Some(Tag::List(l)) => l.clone(),
        _ => Vec::new(),
    };
    for e in list("items") {
        let (Some(key), Some(size)) = (text(e.get("key")), number(e.get("size"))) else {
            continue;
        };
        let size = size as i32;
        let slots = lib::SIZES[usize::try_from(size).ok().filter(|s| *s <= 2).unwrap_or(lib::ITEM_STORAGE_SIZE)];
        let items = horizon_client::save::decode_items(e.get("Items"), slots);
        s.items.insert(
            key,
            lib::ItemStorage::from_saved(size, items, lib::ITEM_STORAGE_SIZE),
        );
    }
    for e in list("liquids") {
        let (Some(key), Some(fluid), Some(amount)) =
            (text(e.get("key")), text(e.get("fluid")), number(e.get("amount")))
        else {
            continue;
        };
        s.liquids.insert(
            key,
            lib::Liquid {
                fluid: Some(fluid),
                amount: (amount as u32).min(lib::TANK_CAPACITY),
            },
        );
    }
    for e in list("blocks") {
        let (Some(dim), Some(x), Some(y), Some(z), Some(kind)) = (
            text(e.get("dim")),
            number(e.get("x")),
            number(e.get("y")),
            number(e.get("z")),
            text(e.get("kind")),
        ) else {
            continue;
        };
        let kind = match kind.as_str() {
            "chest" => lib::Kind::Chest,
            "tank" => lib::Kind::Tank,
            _ => continue,
        };
        let freq = e.get("Frequency").map(lib::frequency_from_tag).unwrap_or_default();
        s.blocks.insert((dim, [x as i32, y as i32, z as i32]), (kind, freq));
    }
    s
}

impl Game<'_> {
    fn es_dim(&self) -> String {
        self.dim.id().to_owned()
    }
    fn es_kind_at(&self, p: [i32; 3]) -> Option<lib::Kind> {
        let name = &self.res.facts().get(self.world.block(p[0], p[1], p[2])).name;
        lib::Kind::of(name)
    }
    fn es_prop(&self, p: [i32; 3], key: &str) -> Option<String> {
        let state = self.world.block(p[0], p[1], p[2]);
        self.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    fn es_rotation(&self, p: [i32; 3]) -> u8 {
        lib::rotation_of_facing(self.es_prop(p, "facing").as_deref().unwrap_or("north"))
    }
    fn es_sneaking(&self) -> bool {
        self.keys.contains(&KeyCode::ShiftLeft)
    }
    fn es_owner(&mut self) -> lib::Owner {
        lib::Owner {
            uuid: self.acting_player_uuid(),
            name: self.acting_player_name().to_owned(),
        }
    }
    /// The frequency of the block at `p` (a block that came from `/setblock` or an old save has
    /// the default one, white three times).
    fn es_known(&mut self, p: [i32; 3], kind: lib::Kind) -> lib::Frequency {
        let dim = self.es_dim();
        with_store(|s| match s.block(&dim, p) {
            Some((k, f)) if *k == kind => f.clone(),
            _ => {
                let f = lib::Frequency::default();
                s.set_block(&dim, p, kind, f.clone());
                f
            }
        })
    }
    /// A new frequency for the block at `p`: its copy takes the new inventory.
    fn es_set_freq(&mut self, p: [i32; 3], kind: lib::Kind, freq: lib::Frequency) {
        let dim = self.es_dim();
        with_store(|s| {
            s.storage(&freq);
            s.set_block(&dim, p, kind, freq);
        });
        with_seen(|seen| seen.remove(&(dim, p)));
    }
    /// The block entity of the chest at `p`, made to the size of the inventory.
    fn es_ensure_be(&mut self, p: [i32; 3]) {
        let key = key_of(p);
        let size = lib::SIZES[lib::ITEM_STORAGE_SIZE];
        match self.world.block_entities.get_mut(&key) {
            Some(BlockEntity::Chest(items)) => {
                if items.len() < size {
                    items.resize(size, None);
                }
            }
            Some(_) => {}
            None => {
                self.world
                    .block_entities
                    .insert(key, BlockEntity::Chest(vec![None; size]));
                self.world.modified.insert((key.0 >> 4, key.2 >> 4));
            }
        }
    }
    fn es_say_give(&mut self, p: [i32; 3], stack: ItemStack) {
        if let Some(rest) = self.give(stack) {
            self.pop_resource(p, rest);
        }
    }
    fn es_sound(&mut self, p: [i32; 3], name: &str) {
        let pitch = (self.world.random.next_double() * 0.1 + 0.9) as f32;
        self.audio.play(
            name,
            Some([
                f64::from(p[0]) + 0.5,
                f64::from(p[1]) + 0.5,
                f64::from(p[2]) + 0.5,
            ]),
            1.0,
            pitch,
        );
    }
    /// Chests never join (`BlockEnderChest` has no `type`): a placed chest the game paired with a
    /// neighbour of its name goes back to single, and so does the neighbour.
    fn es_unpair(&mut self, p: [i32; 3]) {
        for d in [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]] {
            let q = [p[0] + d[0], p[1], p[2] + d[1]];
            if self.es_kind_at(q) != Some(lib::Kind::Chest) {
                continue;
            }
            if self.es_prop(q, "type").is_none_or(|t| t == "single") {
                continue;
            }
            let state = self.world.block(q[0], q[1], q[2]);
            if let Some(fixed) = self.with_props(state, &[("type", "single")]) {
                let at = BlockPos::new(q[0], q[1], q[2]);
                let flags =
                    level_flags::UPDATE_ALL | level_flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS;
                self.with_level(|b, level, ticks| b.set_block(level, ticks, at, fixed, flags));
                let _ = self.process_level_events();
            }
        }
    }

    // ---- placing (trigger_block "minecraft:placed_block", BlockItem.place's end) ----

    pub(super) fn enderstorage_placed(&mut self, trigger: &str, pos: [i32; 3], stack: &ItemStack) {
        if trigger != "minecraft:placed_block" {
            return;
        }
        let Some(kind) = self.es_kind_at(pos) else {
            return;
        };
        // ItemEnderStorage.place: the frequency of the item (`Frequency.readFromStack`).
        let freq = lib::read_frequency(stack);
        if kind == lib::Kind::Chest {
            self.es_unpair(pos);
            self.es_ensure_be(pos);
        }
        self.es_set_freq(pos, kind, freq);
    }

    // ---- clicks (BlockEnderStorage.use) ----

    /// A right click on an Ender Chest or Tank, before the block's own use. The buttons set the
    /// colours with dyes, the latch locks with the personal item (sneak to unlock); the rest of the
    /// tank's use is the bucket, the rest of the chest's the menu (`open_menu`).
    pub(super) fn enderstorage_use(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        let Some(kind) = self.es_kind_at(pos) else {
            return false;
        };
        let Some(hit) = self.pick() else {
            return false;
        };
        if hit.pos != pos {
            return false;
        }
        let freq = self.es_known(pos, kind);
        let local = [
            hit.point[0] - f64::from(pos[0]),
            hit.point[1] - f64::from(pos[1]),
            hit.point[2] - f64::from(pos[2]),
        ];
        let rot = self.es_rotation(pos);
        let sub = match kind {
            lib::Kind::Chest => {
                let closed = self.lid_openers.get(&key_of(pos)).copied().unwrap_or(0) == 0;
                lib::chest_sub_hit(rot, hit.face, local, closed)
            }
            lib::Kind::Tank => lib::tank_sub_hit(rot, hit.face, local),
        };
        let sneaking = self.es_sneaking();
        let who = self.es_owner();
        match lib::use_block(&freq, sub, held.map(|h| &*h.item), sneaking, &who) {
            lib::Use::Recolour { freq, consume } | lib::Use::Lock { freq, consume } => {
                self.es_set_freq(pos, kind, freq);
                if consume && !self.creative {
                    self.consume_held();
                }
                self.swing_arm();
                return true;
            }
            lib::Use::Unlock { freq } => {
                // A full inventory fails the click.
                if !self.creative {
                    let back = ItemStack::new(lib::PERSONAL_ITEM, 1);
                    if self.give(back).is_some() {
                        return true;
                    }
                }
                self.es_set_freq(pos, kind, freq);
                self.swing_arm();
                return true;
            }
            lib::Use::Pass => {}
        }
        // `!player.isCrouching() && owner.activate(..)`
        if sneaking {
            return true;
        }
        match kind {
            lib::Kind::Chest => false,
            lib::Kind::Tank => {
                self.es_tank_use(pos, &freq, sub, held);
                true
            }
        }
    }
    /// `TileEnderTank.activate`: the dial flips the pressure (no pump here), else a bucket.
    fn es_tank_use(&mut self, pos: [i32; 3], freq: &lib::Frequency, sub: u8, held: Option<&ItemStack>) {
        if sub == 4 {
            return;
        }
        let Some(held) = held else {
            return;
        };
        let item = held.item.to_string();
        let result = with_store(|s| {
            let r = lib::bucket_interact(&item, s.liquid(freq));
            if r != lib::Bucket::None {
                s.dirty = true;
            }
            r
        });
        match result {
            lib::Bucket::None => {}
            lib::Bucket::Filled { bucket, sound } => {
                if !self.creative {
                    self.consume_held();
                    self.es_say_give(pos, ItemStack::new(bucket, 1));
                }
                self.es_sound(pos, sound);
                self.swing_arm();
            }
            lib::Bucket::Emptied { sound } => {
                if !self.creative {
                    self.consume_held();
                    self.es_say_give(pos, ItemStack::new(lib::EMPTY_BUCKET, 1));
                }
                self.es_sound(pos, sound);
                self.swing_arm();
            }
        }
    }

    /// `BlockEnderChest.use` after the frequency checks: the menu of the frequency's inventory
    /// (open_menu reaches `open_station` for a block `BlockEntity::for_block` does not know).
    /// The Ender Tank never opens a menu.
    pub(super) fn enderstorage_open(&mut self, pos: [i32; 3], name: &str) -> bool {
        if name == lib::TANK {
            return true;
        }
        if name != lib::CHEST {
            return false;
        }
        let key = key_of(pos);
        self.es_known(pos, lib::Kind::Chest);
        self.es_ensure_be(pos);
        // The copy shows the inventory as it is now.
        self.enderstorage_tick();
        self.container = Some(key);
        let first = self.container_opener_event(key, true);
        self.post_block_game_event(
            "minecraft:container_open",
            pos,
            Some(horizon_client::mobs::Who::Player),
            None,
        );
        if first {
            self.container_edge_sound(key, true);
        }
        true
    }
    /// The open chest or pouch menu: the game draws it as a vanilla ender chest (a generic 9-wide
    /// chest with the inventory's rows); its own title is `enderstorage_title`.
    pub(super) fn enderstorage_container_block(&self) -> Option<std::sync::Arc<str>> {
        let key = self.container?;
        let ours = key == POUCH_KEY
            || lib::Kind::of(&self.res.facts().get(self.world.block(key.0, key.1, key.2)).name)
                == Some(lib::Kind::Chest);
        ours.then(|| std::sync::Arc::from("minecraft:ender_chest"))
    }
    /// The title: the block's (or item's) description.
    pub(super) fn enderstorage_title(&self) -> Option<String> {
        let key = self.container?;
        if key == POUCH_KEY {
            return Some(self.translate("item.enderstorage.ender_pouch", "Ender Pouch"));
        }
        let name = &self.res.facts().get(self.world.block(key.0, key.1, key.2)).name;
        (lib::Kind::of(name) == Some(lib::Kind::Chest))
            .then(|| self.translate("block.enderstorage.ender_chest", "Ender Chest"))
    }

    // ---- the pouch (ItemEnderPouch) ----

    /// `onItemUseFirst` (sneaking at an Ender Chest copies its frequency) and `use` (opens the
    /// frequency's inventory), at the place where books are used: after the blocks reacted.
    pub(super) fn enderstorage_pouch(&mut self, stack: &ItemStack) -> bool {
        if &*stack.item != lib::POUCH {
            return false;
        }
        if self.es_sneaking() {
            let Some(hit) = self.pick() else {
                return false;
            };
            if self.es_kind_at(hit.pos) != Some(lib::Kind::Chest) {
                return false;
            }
            let freq = self.es_known(hit.pos, lib::Kind::Chest);
            if let Some(held) = self.held_mut() {
                lib::write_frequency(held, &freq);
            }
            self.swing_arm();
            return true;
        }
        let freq = lib::read_frequency(stack);
        if self.container.is_some() {
            return false;
        }
        let items = with_store(|s| s.storage(&freq).items.clone());
        self.world
            .block_entities
            .insert(POUCH_KEY, BlockEntity::Chest(items));
        with_seen(|seen| seen.remove(&pouch_seen_key()));
        *POUCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(freq);
        self.container = Some(POUCH_KEY);
        self.screen = Screen::Inventory;
        self.capture(false);
        self.right = false;
        true
    }
    /// The pouch's menu closes: its copy goes to the inventory before the placeholder goes.
    pub(super) fn enderstorage_closing(&mut self, key: (i32, i32, i32)) {
        if key != POUCH_KEY {
            return;
        }
        self.enderstorage_tick();
        *POUCH.lock().unwrap_or_else(|e| e.into_inner()) = None;
        with_seen(|seen| seen.remove(&pouch_seen_key()));
    }

    // ---- keeping the copies equal (TileEnderChest works on the inventory itself) ----

    pub(super) fn enderstorage_tick(&mut self) {
        let dim = self.es_dim();
        let blocks: Vec<([i32; 3], lib::Frequency)> = with_store(|s| {
            s.blocks
                .iter()
                .filter(|((d, _), (k, _))| *d == dim && *k == lib::Kind::Chest)
                .map(|((_, p), (_, f))| (*p, f.clone()))
                .collect()
        });
        let pouch = POUCH.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if blocks.is_empty() && pouch.is_none() {
            return;
        }
        let mut groups: BTreeMap<String, (lib::Frequency, Vec<[i32; 3]>)> = BTreeMap::new();
        for (p, f) in blocks {
            if matches!(self.world.block_entities.get(&key_of(p)), Some(BlockEntity::Chest(_))) {
                groups.entry(f.key()).or_insert_with(|| (f, Vec::new())).1.push(p);
            }
        }
        if let Some(f) = &pouch {
            groups.entry(f.key()).or_insert_with(|| (f.clone(), Vec::new()));
        }
        for (_, (freq, places)) in groups {
            // (key, seen-key, items, fresh)
            let mut taken: Vec<((i32, i32, i32), (String, [i32; 3]), Vec<Option<ItemStack>>, bool)> =
                Vec::new();
            let mut wanted: Vec<((i32, i32, i32), (String, [i32; 3]))> =
                places.iter().map(|p| (key_of(*p), (dim.clone(), *p))).collect();
            if pouch.as_ref().is_some_and(|f| f.key() == freq.key()) {
                wanted.push((POUCH_KEY, pouch_seen_key()));
            }
            for (key, seen_key) in wanted {
                match self.world.block_entities.remove(&key) {
                    Some(BlockEntity::Chest(items)) => {
                        let fresh = !with_seen(|seen| seen.contains(&seen_key));
                        taken.push((key, seen_key, items, fresh));
                    }
                    Some(other) => {
                        self.world.block_entities.insert(key, other);
                    }
                    None => {}
                }
            }
            let changed = {
                let mut works: Vec<lib::Working<'_, ItemStack>> = taken
                    .iter_mut()
                    .map(|(_, _, items, fresh)| lib::Working {
                        items,
                        fresh: *fresh,
                    })
                    .collect();
                with_store(|s| s.reconcile(&freq, &mut works))
            };
            for (key, seen_key, items, _) in taken {
                self.world.block_entities.insert(key, BlockEntity::Chest(items));
                if changed && key.1 != super::minecarts::CART_KEY_Y {
                    self.world.modified.insert((key.0 >> 4, key.2 >> 4));
                }
                with_seen(|seen| seen.insert(seen_key));
            }
        }
    }

    // ---- breaking (BlockEnderStorage.getDrops: the item keeps the frequency) ----

    pub(super) fn enderstorage_loot(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
        value: Vec<ItemStack>,
    ) -> Vec<ItemStack> {
        let Some(kind) = lib::Kind::of(&self.res.facts().get(state).name) else {
            return value;
        };
        let dim = self.es_dim();
        let freq = with_store(|s| s.remove_block(&dim, pos))
            .map(|(_, f)| f)
            .unwrap_or_default();
        with_seen(|seen| seen.remove(&(dim, pos)));
        // `createItem`: anarchyMode takes the owner off.
        let mut freq = freq;
        if lib::ANARCHY_MODE {
            freq.owner = None;
        }
        let mut stack = ItemStack::new(kind.block(), 1);
        lib::write_frequency(&mut stack, &freq);
        let mut out = vec![stack];
        if lib::ANARCHY_MODE {
            out.push(ItemStack::new(lib::PERSONAL_ITEM, 1));
        }
        out
    }

    // ---- crafting (CreateRecipe, ReColourRecipe) ----

    pub(super) fn enderstorage_craft(&self) -> Option<(String, ItemStack)> {
        let n = self.craft_size;
        let grid: Vec<Option<ItemStack>> = (0..n * n).map(|i| self.craft_grid[i].clone()).collect();
        lib::crafting(&grid, n).map(|(name, stack)| (name.to_owned(), stack))
    }

    // ---- the world file ----

    pub(super) fn enderstorage_load(&mut self) {
        // The Ender Tank is a fluid handler for pipes and machines (forja, enabled with this mod).
        crate::game::forja::register_fluid("enderstorage", es_tank_claims, es_tank_fluid);
        let path = self.world_root.join("data/enderstorage/store.nbt");
        let loaded = std::fs::read(path)
            .ok()
            .and_then(|bytes| read_named(&bytes, Limits::default()).ok())
            .map(|(named, _)| store_from_tag(&named.tag))
            .unwrap_or_default();
        with_store(|s| *s = loaded);
        with_seen(|seen| seen.clear());
        *POUCH.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    pub(super) fn enderstorage_save(&mut self) {
        if self.world_root.as_os_str().is_empty() {
            return;
        }
        // Make the inventories current (a hopper may have run since the last tick).
        self.enderstorage_tick();
        let dim = self.es_dim();
        // Forget blocks that are gone: only in loaded chunks of this dimension.
        let gone: Vec<(String, [i32; 3])> = with_store(|s| {
            s.blocks
                .keys()
                .filter(|(d, p)| {
                    *d == dim
                        && self.world.chunks.get(&(p[0] >> 4, p[2] >> 4)).is_some()
                        && self.es_kind_at(*p).is_none()
                })
                .cloned()
                .collect()
        });
        let dir = self.world_root.join("data/enderstorage");
        let file = dir.join("store.nbt");
        let tag = with_store(|s| {
            for g in &gone {
                s.remove_block(&g.0, g.1);
            }
            if !s.dirty && file.is_file() {
                return None;
            }
            s.dirty = false;
            Some(store_to_tag(s))
        });
        let Some(tag) = tag else {
            return;
        };
        let root = NamedTag {
            name: JavaString::from(""),
            tag,
        };
        let Ok(bytes) = write_named(&root, Limits::default()) else {
            return;
        };
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let tmp = dir.join("store.nbt.tmp");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &file);
        }
    }

    /// The chest item's model sprite, put in the GUI atlas's names after each (re)load of it.
    pub(super) fn enderstorage_sprites(&mut self) {
        let sprites = &mut self.gui.atlas.sprites;
        if sprites.contains_key(ITEM_SPRITE) {
            return;
        }
        if let Some(rect) = sprites.get(lib::CHEST_TEXTURE).copied() {
            sprites.insert(ITEM_SPRITE.to_owned(), rect);
        }
    }
}

// ---- the Ender Tank as a fluid handler (TileEnderTank.getCapability(FLUID_HANDLER), any side) ----

fn es_tank_claims(game: &Game<'_>, p: [i32; 3], _face: Option<crate::game::forja::Dir>) -> bool {
    game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)) && game.es_kind_at(p) == Some(lib::Kind::Tank)
}

/// The frequency's tank (`EnderLiquidStorage`, 16 buckets) as a forja `FluidTank` for one operation;
/// what the operation changed is written back to the frequency.
fn es_tank_fluid(
    game: &mut Game<'_>,
    p: [i32; 3],
    _face: Option<crate::game::forja::Dir>,
    f: &mut dyn FnMut(&mut dyn crate::game::forja::FluidHandler),
) -> bool {
    use crate::game::forja::{FluidStack, FluidTank};
    if !es_tank_claims(game, p, None) {
        return false;
    }
    let freq = game.es_known(p, lib::Kind::Tank);
    with_store(|s| {
        let before = s.liquid(&freq).clone();
        let mut tank = FluidTank::new(lib::TANK_CAPACITY as i32);
        if let Some(fluid) = &before.fluid
            && before.amount > 0
        {
            tank.fluid = FluidStack::new(fluid, before.amount as i32);
        }
        f(&mut tank);
        let after = if tank.fluid.is_empty() {
            lib::Liquid::default()
        } else {
            lib::Liquid {
                fluid: Some(tank.fluid.fluid.clone()),
                amount: tank.fluid.amount as u32,
            }
        };
        if after != before {
            *s.liquid(&freq) = after;
            s.dirty = true;
        }
    });
    true
}
