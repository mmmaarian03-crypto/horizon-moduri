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
    // Hats Renewed (hats) for Horizon: the rules, without any game code.
// Written from the pack's 1.20.1 jar `HatsRenewed-20.1.2-mc1.20.1-forge.jar` (decompiled in surse/hats/dec; LGPL):
// HatRegistry, HatDefinition, HatPart, HatPool, HatRarity, HatsSavedData, PlayerHatData, ServerEventHandler,
// HatEntity, HatLauncherItem, HatPlacementRegistry, LayerHat, HatToast, HatsScreen; and `config/hats.toml`.
// Pasted into the game as `crate::game::hats::engine` (tools/hnmod.py bundle); also the `horizon_hats` crate of the tests.

use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// the pack's `config/hats.toml` (and `HatsConfig`'s defaults for what the file does not set).
pub mod config {
    pub const HAT_BLACKLIST: &[&str] = &[]; // hats.toml [server] hat_blacklist
    pub const DISABLED_ENTITIES: &[&str] = &[]; // hats.toml [server] disabled_entities
    pub const HATS_SHOW_WHEN_INVISIBLE: bool = false; // hats.toml [server] hats_show_when_invisible
    pub const HATS_PREVENT_UNDEAD_FIRE: bool = false; // hats.toml [server] hats_prevent_undead_fire
    pub const DISABLE_CONTRIBUTOR_HATS: bool = false; // hats.toml [server] disable_contributor_hats
    pub const HAT_UNLOCK_CHAT_ANNOUNCEMENT: bool = false; // hats.toml [server] hat_unlock_chat_announcement
    /// [server.spawn_chance] common, uncommon, rare, epic, legendary.
    pub const SPAWN_CHANCE: [f64; 5] = [0.1, 0.08, 0.05, 0.03, 0.02];
    pub const MAX_HATS_ON_SCREEN: usize = 250; // hats.toml [client] max_hats_on_screen
    pub const DISPLAY_HAT_UNLOCK_TOAST: bool = true; // hats.toml [client] display_hat_unlock_toast
    pub const DISPLAY_OPTIONS_BUTTON: bool = true; // hats.toml [client] display_options_button
    // HatsConfig defaults (not in the pack's file).
    pub const RAND_SEED: &str = ""; // HatsConfig.randSeed
    pub const RARITY_WEIGHTS: [i32; 5] = [21, 13, 8, 5, 3]; // HatsConfig.rarityWeights
    pub const RARITY_INDIVIDUAL: [f64; 5] = [0.21, 0.13, 0.08, 0.05, 0.03]; // HatsConfig.rarityIndividual
    pub const MOB_HAT_TAKEOVER: bool = false; // HatsConfig.mobHatTakeover
    pub const HAT_ENTITY_LIFESPAN: i32 = 6000; // HatsConfig.hatEntityLifespan
    pub const HAT_LAUNCHER_REPLACES_HAT: bool = true; // HatsConfig.hatLauncherReplacesHat
}

// ---------------------------------------------------------------------------------------------------------------
// Java pieces the rolls depend on

/// `java.util.Random` (the 48-bit LCG).
#[derive(Clone, Debug)]
pub struct JavaRandom(i64);
impl JavaRandom {
    const MULT: i64 = 0x5DEE_CE66D;
    pub fn new(seed: i64) -> Self {
        JavaRandom((seed ^ Self::MULT) & ((1 << 48) - 1))
    }
    pub fn set_seed(&mut self, seed: i64) {
        *self = Self::new(seed);
    }
    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(Self::MULT).wrapping_add(0xB)) & ((1 << 48) - 1);
        (self.0 >> (48 - bits)) as i32
    }
    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if bound & -bound == bound {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }
    pub fn next_double(&mut self) -> f64 {
        let hi = i64::from(self.next(26));
        let lo = i64::from(self.next(27));
        ((hi << 27) + lo) as f64 * (1.0 / (1i64 << 53) as f64)
    }
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }
}

/// `String.hashCode()` (UTF-16 code units).
pub fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// `(long)Math.abs(int)`: `Integer.MIN_VALUE` stays negative.
fn abs_long(h: i32) -> i64 {
    i64::from(h.wrapping_abs())
}

/// `UUID.toString()` of the four NBT ints (most significant first).
pub fn uuid_string(u: [i32; 4]) -> String {
    let h = format!("{:08x}{:08x}{:08x}{:08x}", u[0] as u32, u[1] as u32, u[2] as u32, u[3] as u32);
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// `UUID.hashCode()`.
pub fn uuid_hash(u: [i32; 4]) -> i32 {
    let most = (i64::from(u[0]) << 32) | i64::from(u[1] as u32);
    let least = (i64::from(u[2]) << 32) | i64::from(u[3] as u32);
    let hilo = most ^ least;
    ((hilo >> 32) as i32) ^ (hilo as i32)
}

/// The four ints of a u128 UUID (a network player's).
pub fn uuid_ints(u: u128) -> [i32; 4] {
    [(u >> 96) as u32 as i32, (u >> 64) as u32 as i32, (u >> 32) as u32 as i32, u as u32 as i32]
}

// ---------------------------------------------------------------------------------------------------------------
// Hats

/// `HatRarity`, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}
pub const RARITIES: [Rarity; 5] = [Rarity::Common, Rarity::Uncommon, Rarity::Rare, Rarity::Epic, Rarity::Legendary];
impl Rarity {
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn from_name(s: &str) -> Option<Rarity> {
        RARITIES.iter().copied().find(|r| r.name().eq_ignore_ascii_case(s))
    }
    pub fn name(self) -> &'static str {
        ["common", "uncommon", "rare", "epic", "legendary"][self.index()]
    }
    /// `HatRarity.colour` as an ARGB text colour (WHITE, GREEN, AQUA, LIGHT_PURPLE, GOLD).
    pub fn colour(self) -> u32 {
        [0xFFFF_FFFF, 0xFF55_FF55, 0xFF55_FFFF, 0xFFFF_55FF, 0xFFFF_AA00][self.index()]
    }
    /// `HatRarity.defaultWorth`.
    pub fn worth(self) -> i32 {
        [10, 20, 40, 70, 110][self.index()]
    }
}

/// `HatRegistry.rebuildThresholds` / `getRarityForChance` with the configured weights.
pub fn rarity_for_chance(chance: f64) -> Rarity {
    let total: i32 = config::RARITY_WEIGHTS.iter().sum();
    let mut cumulative = 0.0f64;
    for r in RARITIES {
        cumulative += f64::from(config::RARITY_WEIGHTS[r.index()]) / f64::from(total);
        if chance <= cumulative {
            return r;
        }
    }
    Rarity::Legendary
}

/// One `HatDefinition` (a hat or an accessory), from the carry's `hats-index.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct HatDef {
    /// The index key (`adventurers_hat/forgotten_adventurer`): the item model `hats:item/hat/<key>`.
    pub key: String,
    pub name: String,
    pub forced_rarity: Option<Rarity>,
    pub forced_pool: Option<String>,
    pub forced_worth: Option<i32>,
    pub contributor: bool,
    pub accessory_for: Option<String>,
    pub accessory_parent: Option<String>,
    pub accessory_layers: Vec<String>,
    pub item_model: String,
    /// The item definition to draw it with (`hats:hat.<key>`: Horizon reads `items/*.json` only at the top).
    pub item: String,
    pub order: usize,
    /// Indices of the accessories attached to it (`attachAccessories`), in order.
    pub accessories: Vec<usize>,
    pub rarity: Rarity,
}
impl HatDef {
    /// `getFullName`: `accessoryFor:` `accessoryParent|` name.
    pub fn full_name(&self) -> String {
        let mut s = String::new();
        if let Some(a) = &self.accessory_for {
            s.push_str(a);
            s.push(':');
        }
        if let Some(p) = &self.accessory_parent {
            s.push_str(p);
            s.push('|');
        }
        s.push_str(&self.name);
        s
    }
    pub fn is_accessory(&self) -> bool {
        self.accessory_for.is_some()
    }
    /// `getWorth`: forced, else the rarity's tokens (x1.5 rounded up for accessories).
    pub fn worth(&self) -> i32 {
        match self.forced_worth {
            Some(w) => w,
            None if self.is_accessory() => (1.5 * f64::from(self.rarity.worth())).ceil() as i32,
            None => self.rarity.worth(),
        }
    }
}

/// `HatRegistry`: every hat and accessory by full name, the pools by rarity.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub defs: Vec<HatDef>,
    pub by_full_name: BTreeMap<String, usize>,
    /// Per rarity: the pools (name, hat indices) in creation order.
    pub pools: [Vec<(String, Vec<usize>)>; 5],
}

impl Registry {
    /// Build from `hats-index.json` as `HatLoader.loadHats` does: load order, accessories attached to their hat
    /// (`attachAccessories`), registration of hats then their accessories, pools (`buildPools`).
    pub fn from_index(index: &Value) -> Registry {
        let mut all: Vec<HatDef> = Vec::new();
        for (key, v) in index.as_object().into_iter().flatten() {
            let meta = &v["meta"];
            let s = |k: &str| meta[k].as_str().map(str::to_owned).filter(|x| !x.is_empty());
            all.push(HatDef {
                key: key.clone(),
                name: v["name"].as_str().unwrap_or(key).to_owned(),
                forced_rarity: meta["rarity"].as_str().and_then(Rarity::from_name),
                forced_pool: s("pool"),
                forced_worth: meta["worth"].as_i64().map(|w| w as i32),
                contributor: meta["contributor"].as_bool().unwrap_or(false),
                accessory_for: s("accessory_for"),
                accessory_parent: s("accessory_parent"),
                accessory_layers: meta["accessory_layers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect(),
                item_model: v["item_model"].as_str().unwrap_or_default().to_owned(),
                item: v["item_flat"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("hats:hat.{}", key.replace('/', "."))),
                order: v["order"].as_u64().unwrap_or(u64::MAX) as usize,
                accessories: Vec::new(),
                rarity: Rarity::Common,
            });
        }
        all.sort_by_key(|d| d.order);
        // deduplicate by full name (first wins), then hats / accessories.
        let mut seen = BTreeSet::new();
        all.retain(|d| seen.insert(d.full_name()));
        let (hats, mut candidates): (Vec<HatDef>, Vec<HatDef>) = all.into_iter().partition(|d| !d.is_accessory());
        let mut reg = Registry::default();
        for hat in hats {
            if config::DISABLE_CONTRIBUTOR_HATS && hat.contributor {
                continue;
            }
            let root_name = hat.name.clone();
            let i = reg.push(hat);
            reg.attach(i, &mut candidates, &root_name);
        }
        for (i, d) in reg.defs.iter_mut().enumerate() {
            d.rarity = d.forced_rarity.unwrap_or_else(|| {
                let seed = abs_long(java_hash(&format!("{}{}", config::RAND_SEED, d.full_name())))
                    .wrapping_mul(154_041_013);
                rarity_for_chance(JavaRandom::new(seed).next_double())
            });
            let _ = i;
        }
        // buildPools: non-accessory hats in registration order.
        let mut named: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for i in 0..reg.defs.len() {
            let d = &reg.defs[i];
            if d.is_accessory() {
                continue;
            }
            let pool_name = d.forced_pool.clone().unwrap_or_else(|| d.name.clone());
            let r = d.rarity.index();
            let key = format!("{}:{pool_name}", d.rarity.name().to_uppercase());
            let slot = *named.entry(key).or_insert_with(|| {
                reg.pools[r].push((pool_name.clone(), Vec::new()));
                (r, reg.pools[r].len() - 1)
            });
            reg.pools[slot.0][slot.1].1.push(i);
        }
        reg
    }
    fn push(&mut self, d: HatDef) -> usize {
        let i = self.defs.len();
        self.by_full_name.entry(d.full_name()).or_insert(i);
        self.defs.push(d);
        i
    }
    /// `attachAccessories(candidates, rootName)`, registering each attached accessory right after (as
    /// `registerAccessoriesRecursively` does once the whole tree is built).
    fn attach(&mut self, i: usize, candidates: &mut Vec<HatDef>, root_name: &str) {
        let name = self.defs[i].name.clone();
        let mut claimed = Vec::new();
        let mut k = 0;
        while k < candidates.len() {
            let c = &candidates[k];
            let parent_match = c.accessory_parent.as_ref().is_none_or(|p| *p == name);
            if c.accessory_for.as_deref() == Some(root_name) && parent_match {
                claimed.push(candidates.remove(k));
            } else {
                k += 1;
            }
        }
        let mut children = Vec::new();
        for c in claimed {
            children.push(self.push(c));
        }
        self.defs[i].accessories = children.clone();
        for c in children {
            self.attach(c, candidates, root_name);
        }
    }
    pub fn get(&self, full_name: &str) -> Option<&HatDef> {
        self.by_full_name.get(full_name).map(|&i| &self.defs[i])
    }
    /// The definition a part names (`HatRegistry.get(part.getRegistryKey())`).
    pub fn def_of(&self, part: &HatPart) -> Option<&HatDef> {
        self.get(part.registry_key())
    }
    /// The root hat named `name` (inventories key hats by name).
    pub fn hat_named(&self, name: &str) -> Option<&HatDef> {
        self.get(name).filter(|d| !d.is_accessory())
    }
}

/// `HatPart`: a hat as worn, owned or thrown.
#[derive(Clone, Debug, PartialEq)]
pub struct HatPart {
    pub name: String,
    pub registry_key: Option<String>,
    pub count: i32,
    pub favourite: bool,
    pub is_new: bool,
    pub showing: bool,
    pub enchanted: bool,
    pub colour: [f32; 4],
    pub hsb: [f32; 3],
    pub accessories: Vec<HatPart>,
}
impl HatPart {
    pub fn new(name: &str) -> Self {
        HatPart {
            name: name.to_owned(),
            registry_key: None,
            count: 1,
            favourite: false,
            is_new: false,
            showing: false,
            enchanted: false,
            colour: [0.0; 4],
            hsb: [0.0; 3],
            accessories: Vec::new(),
        }
    }
    pub fn registry_key(&self) -> &str {
        self.registry_key.as_deref().unwrap_or(&self.name)
    }
    pub fn has_accessory(&self, name: &str) -> bool {
        self.accessories.iter().any(|a| a.name == name)
    }
    /// `HatPart.save` as JSON (the same keys as the NBT).
    pub fn to_json(&self) -> Value {
        let mut o = json!({
            "name": self.name, "count": self.count, "favourite": self.favourite, "new": self.is_new,
            "showing": self.showing, "enchanted": self.enchanted,
            "colR": self.colour[0], "colG": self.colour[1], "colB": self.colour[2], "colA": self.colour[3],
            "hsbH": self.hsb[0], "hsbS": self.hsb[1], "hsbB": self.hsb[2],
        });
        if let Some(k) = &self.registry_key {
            o["registryKey"] = json!(k);
        }
        if !self.accessories.is_empty() {
            o["accessories"] = Value::Array(self.accessories.iter().map(HatPart::to_json).collect());
        }
        o
    }
    pub fn from_json(v: &Value) -> HatPart {
        let f = |k: &str| v[k].as_f64().unwrap_or(0.0) as f32;
        let b = |k: &str| v[k].as_bool().unwrap_or(false);
        HatPart {
            name: v["name"].as_str().unwrap_or_default().to_owned(),
            registry_key: v["registryKey"].as_str().map(str::to_owned),
            count: v["count"].as_i64().unwrap_or(0) as i32,
            favourite: b("favourite"),
            is_new: b("new"),
            showing: b("showing"),
            enchanted: b("enchanted"),
            colour: [f("colR"), f("colG"), f("colB"), f("colA")],
            hsb: [f("hsbH"), f("hsbS"), f("hsbB")],
            accessories: v["accessories"].as_array().into_iter().flatten().map(HatPart::from_json).collect(),
        }
    }
}

/// `HatDefinition.asHatPart(count)`: the hat with all its accessories, hidden.
pub fn as_hat_part(reg: &Registry, i: usize, count: i32) -> HatPart {
    let d = &reg.defs[i];
    let mut part = HatPart::new(&d.name);
    part.registry_key = Some(d.full_name());
    part.showing = true;
    part.count = count;
    for &a in &d.accessories {
        let mut child = as_hat_part(reg, a, count);
        child.showing = false;
        part.accessories.push(child);
    }
    part
}

/// `HatsSavedData.isEntityDisabled`: a full id or a namespace.
pub fn entity_disabled(entity: &str) -> bool {
    let ns = entity.split(':').next().unwrap_or(entity);
    config::DISABLED_ENTITIES.iter().any(|e| *e == entity || *e == ns)
}

/// `HatsSavedData.assignMobHat` for a mob that has no hat yet: the hat it spawns with, a pure function of its
/// UUID (the roll is seeded with `randSeed + uuid`), or none.
pub fn roll_mob_hat(reg: &Registry, uuid: [i32; 4], entity: &str) -> Option<HatPart> {
    if entity_disabled(entity) {
        return None;
    }
    let uuid_s = uuid_string(uuid);
    let mut rand = JavaRandom::new(abs_long(java_hash(&format!("{}{uuid_s}", config::RAND_SEED))).wrapping_mul(425_480_085));
    // rollRarity: no per-entity overrides in the pack's file, so the rate scale is 1.
    let roll = rand.next_double();
    let mut cumulative = 0.0;
    let mut rarity = None;
    for r in RARITIES {
        cumulative += config::SPAWN_CHANCE[r.index()];
        if roll < cumulative {
            rarity = Some(r);
            break;
        }
    }
    let rarity = rarity?;
    let pools = &reg.pools[rarity.index()];
    let candidates: Vec<usize> = pools
        .iter()
        .flat_map(|(_, hats)| hats.iter().copied())
        .filter(|&i| {
            let d = &reg.defs[i];
            !config::HAT_BLACKLIST.contains(&d.name.as_str()) && !(config::DISABLE_CONTRIBUTOR_HATS && d.contributor)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let i = candidates[rand.next_int(candidates.len() as i32) as usize];
    let mut hat = HatPart::new(&reg.defs[i].name);
    hat.showing = true;
    hat.count = 1;
    attach_accessories_to_part(reg, i, &mut hat, uuid, &uuid_s);
    Some(hat)
}

/// `HatDefinition.attachAccessoriesToPart`: each accessory rolls its own chance, accessories on the same layer
/// keep one (`new Random(uuid.hashCode())` picks the losers).
pub fn attach_accessories_to_part(reg: &Registry, i: usize, part: &mut HatPart, uuid: [i32; 4], uuid_s: &str) {
    let d = &reg.defs[i];
    let mut spawning: Vec<usize> = Vec::new();
    let mut conflicts: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut rand = JavaRandom::new(0);
    for &a in &d.accessories {
        let acc = &reg.defs[a];
        let seed = abs_long(java_hash(&format!("{}{uuid_s}{}", config::RAND_SEED, acc.full_name())))
            .wrapping_mul(53_579_997_854);
        rand.set_seed(seed);
        let chance = config::RARITY_INDIVIDUAL[acc.rarity.index()];
        if rand.next_double() < chance {
            spawning.push(a);
            for layer in &acc.accessory_layers {
                conflicts.entry(layer.clone()).or_default().push(a);
            }
        }
    }
    conflicts.retain(|_, v| v.len() > 1);
    for list in conflicts.values_mut() {
        let mut tie = JavaRandom::new(i64::from(uuid_hash(uuid)));
        while list.len() > 1 {
            let loser = list[tie.next_int(list.len() as i32) as usize];
            spawning.retain(|&s| s != loser);
            list.retain(|&s| s != loser);
        }
    }
    for a in spawning {
        let acc = &reg.defs[a];
        let mut p = HatPart::new(&acc.name);
        p.registry_key = Some(acc.full_name());
        p.showing = true;
        attach_accessories_to_part(reg, a, &mut p, uuid, uuid_s);
        part.accessories.push(p);
    }
}

/// The definitions a worn part draws: itself and each showing accessory, recursively (`HatRenderer.render`).
pub fn drawn_parts<'r>(reg: &'r Registry, part: &HatPart, out: &mut Vec<&'r HatDef>) {
    if !part.showing {
        return;
    }
    let Some(d) = reg.def_of(part).or_else(|| reg.get(&part.name)) else {
        return;
    };
    out.push(d);
    for a in &part.accessories {
        drawn_parts(reg, a, out);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Player data

/// `PlayerHatData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerData {
    pub tokens: i32,
    pub equipped: Option<HatPart>,
    pub inventory: Vec<HatPart>,
}
impl PlayerData {
    pub fn has_hat(&self, name: &str) -> bool {
        self.inventory.iter().any(|p| p.name == name && p.count > 0)
    }
    /// `addHat`.
    pub fn add_hat(&mut self, mut incoming: HatPart) {
        if let Some(existing) = self.inventory.iter_mut().find(|e| e.name == incoming.name) {
            existing.count += incoming.count;
            existing.is_new = true;
            for acc in &incoming.accessories {
                if !existing.has_accessory(&acc.name) {
                    let mut copy = HatPart::new(&acc.name);
                    copy.showing = false;
                    existing.accessories.push(copy);
                }
            }
            return;
        }
        for acc in &mut incoming.accessories {
            acc.showing = false;
        }
        incoming.is_new = true;
        self.inventory.push(incoming);
    }
    /// `removeOne`.
    pub fn remove_one(&mut self, name: &str) -> bool {
        if let Some(i) = self.inventory.iter().position(|p| p.name == name && p.count > 0) {
            self.inventory[i].count -= 1;
            if self.inventory[i].count <= 0 {
                self.inventory.remove(i);
            }
            return true;
        }
        false
    }
    /// `takeRandomNonFavourite`.
    pub fn take_random_non_favourite(&mut self, rand: &mut JavaRandom) -> Option<HatPart> {
        let available: Vec<usize> =
            (0..self.inventory.len()).filter(|&i| self.inventory[i].count > 0 && !self.inventory[i].favourite).collect();
        if available.is_empty() {
            return None;
        }
        let chosen = available[rand.next_int(available.len() as i32) as usize];
        let mut result = self.inventory[chosen].clone();
        let name = result.name.clone();
        self.remove_one(&name);
        result.count = 1;
        Some(result)
    }
    /// `takeSpecific`.
    pub fn take_specific(&mut self, name: &str) -> Option<HatPart> {
        let p = self.inventory.iter().find(|p| p.name == name && p.count > 0 && !p.favourite)?;
        let mut result = p.clone();
        self.remove_one(name);
        result.count = 1;
        Some(result)
    }
    pub fn to_json(&self, owner: &str) -> Value {
        let mut o = json!({ "owner": owner, "tokens": self.tokens,
            "inventory": self.inventory.iter().map(HatPart::to_json).collect::<Vec<_>>() });
        if let Some(e) = &self.equipped {
            o["equipped"] = e.to_json();
        }
        o
    }
    pub fn from_json(v: &Value) -> PlayerData {
        PlayerData {
            tokens: v["tokens"].as_i64().unwrap_or(0) as i32,
            equipped: v.get("equipped").filter(|e| e.is_object()).map(HatPart::from_json),
            inventory: v["inventory"].as_array().into_iter().flatten().map(HatPart::from_json).collect(),
        }
    }
}

/// A "New Hat Unlocked!" toast (`PacketNewHatToast`): the part shown, whether it is an accessory.
#[derive(Clone, Debug, PartialEq)]
pub struct Unlock {
    pub hat: HatPart,
    pub accessory: bool,
}

/// `HatsSavedData`: every player's data and the hats entities wear that are not their spawn roll.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Save {
    pub players: BTreeMap<String, PlayerData>,
    /// UUID string -> hat (`entityHats`); `None` = the entity's hat was taken (it died, it was replaced).
    pub entity_hats: BTreeMap<String, Option<HatPart>>,
}
impl Save {
    pub fn player(&mut self, uuid: &str) -> &mut PlayerData {
        self.players.entry(uuid.to_owned()).or_default()
    }
    /// `ServerEventHandler.onPlayerPickupHat`: the hat goes into the collection; the toasts to show.
    pub fn pickup(&mut self, uuid: &str, hat: HatPart) -> Vec<Unlock> {
        let data = self.player(uuid);
        let is_new = !data.has_hat(&hat.name);
        let owned: BTreeSet<String> = data
            .inventory
            .iter()
            .find(|p| p.name == hat.name)
            .map(|p| p.accessories.iter().map(|a| a.name.clone()).collect())
            .unwrap_or_default();
        let incoming: Vec<HatPart> = hat.accessories.clone();
        let shown = HatPart::new(&hat.name);
        data.add_hat(hat);
        if is_new {
            return vec![Unlock { hat: shown, accessory: false }];
        }
        incoming
            .into_iter()
            .filter(|a| !owned.contains(&a.name))
            .map(|a| Unlock { hat: a, accessory: true })
            .collect()
    }
    pub fn to_json(&self) -> Value {
        let players: Vec<Value> = self.players.iter().map(|(u, d)| d.to_json(u)).collect();
        let entities: Vec<Value> = self
            .entity_hats
            .iter()
            .map(|(u, h)| match h {
                Some(h) => json!({"uuid": u, "hat": h.to_json()}),
                None => json!({"uuid": u}),
            })
            .collect();
        json!({ "players": players, "entityHats": entities })
    }
    pub fn from_json(v: &Value) -> Save {
        let mut s = Save::default();
        for p in v["players"].as_array().into_iter().flatten() {
            if let Some(owner) = p["owner"].as_str() {
                s.players.insert(owner.to_owned(), PlayerData::from_json(p));
            }
        }
        for e in v["entityHats"].as_array().into_iter().flatten() {
            if let Some(u) = e["uuid"].as_str() {
                s.entity_hats.insert(u.to_owned(), e.get("hat").filter(|h| h.is_object()).map(HatPart::from_json));
            }
        }
        s
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Where a hat sits

/// `HatPlacementRegistry.registerVanilla`: (scale, boss) per entity type; everything else `DEFAULT` (1, no).
pub fn placement(entity: &str) -> (f32, bool) {
    match entity {
        "minecraft:horse" | "minecraft:polar_bear" => (1.2, false),
        "minecraft:donkey" | "minecraft:mule" | "minecraft:llama" | "minecraft:trader_llama" | "minecraft:snow_golem" => {
            (1.1, false)
        }
        "minecraft:iron_golem" => (1.5, false),
        "minecraft:bee" => (0.9, false),
        "minecraft:rabbit" => (0.8, false),
        "minecraft:elder_guardian" | "minecraft:wither" => (1.5, true),
        "minecraft:ender_dragon" => (4.0, true),
        "minecraft:ravager" => (1.8, true),
        _ => (1.0, false),
    }
}

/// `LayerHat.computeHeadOffsets` / `offsetsFromCubes` on the head part's own boxes (pixels, part space, y down):
/// the top centre of the main box and its width / 8 clamped to 0.2..3. Each box is (min, max). `None` when the
/// head has no boxes.
pub fn head_offsets(boxes: &[([f32; 3], [f32; 3])]) -> Option<([f32; 3], f32)> {
    let volume = |b: &([f32; 3], [f32; 3])| (b.1[0] - b.0[0]) * (b.1[1] - b.0[1]) * (b.1[2] - b.0[2]);
    let mut primary: Option<&([f32; 3], [f32; 3])> = None;
    for c in boxes {
        if volume(c) >= 32.0
            && primary.is_none_or(|p| c.0[1] < p.0[1] || (c.0[1] == p.0[1] && volume(c) > volume(p)))
        {
            primary = Some(c);
        }
    }
    if let Some(p) = primary {
        let scale = ((p.1[0] - p.0[0]) / 8.0).clamp(0.2, 3.0);
        return Some(([(p.0[0] + p.1[0]) / 2.0 / 16.0, p.0[1] / 16.0, (p.0[2] + p.1[2]) / 2.0 / 16.0], scale));
    }
    if boxes.is_empty() {
        return None;
    }
    let top = boxes.iter().map(|b| b.0[1]).fold(f32::MAX, f32::min);
    let (mut sx, mut sz, mut minx, mut maxx, mut n) = (0.0, 0.0, f32::MAX, -f32::MAX, 0);
    for c in boxes.iter().filter(|c| c.0[1] <= top + 2.0) {
        sx += (c.0[0] + c.1[0]) / 2.0;
        sz += (c.0[2] + c.1[2]) / 2.0;
        minx = minx.min(c.0[0]);
        maxx = maxx.max(c.1[0]);
        n += 1;
    }
    let (cx, cz) = if n > 0 { (sx / n as f32, sz / n as f32) } else { (0.0, 0.0) };
    let span = if maxx > minx { maxx - minx } else { 8.0 };
    Some(([cx / 16.0, top / 16.0, cz / 16.0], (span / 8.0).clamp(0.2, 3.0)))
}

/// The boxes of a baked part from its quads: each axis-aligned box is rebuilt from a pair of horizontal faces
/// with the same x/z extent (its top and bottom), a box flat in y from one face.
pub fn boxes_from_quads(quads: &[[[f32; 3]; 4]]) -> Vec<([f32; 3], [f32; 3])> {
    let mut flats: Vec<([f32; 3], [f32; 3])> = Vec::new();
    for q in quads {
        let y = q[0][1];
        if q.iter().any(|v| (v[1] - y).abs() > 1e-4) {
            continue;
        }
        let min = [q.iter().map(|v| v[0]).fold(f32::MAX, f32::min), y, q.iter().map(|v| v[2]).fold(f32::MAX, f32::min)];
        let max = [q.iter().map(|v| v[0]).fold(-f32::MAX, f32::max), y, q.iter().map(|v| v[2]).fold(-f32::MAX, f32::max)];
        flats.push((min, max));
    }
    let mut out: Vec<([f32; 3], [f32; 3])> = Vec::new();
    let mut used = vec![false; flats.len()];
    for i in 0..flats.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let a = flats[i];
        let same = |b: &([f32; 3], [f32; 3])| {
            (a.0[0] - b.0[0]).abs() < 1e-4
                && (a.0[2] - b.0[2]).abs() < 1e-4
                && (a.1[0] - b.1[0]).abs() < 1e-4
                && (a.1[2] - b.1[2]).abs() < 1e-4
        };
        let mate = (i + 1..flats.len()).find(|&j| !used[j] && same(&flats[j]) && (flats[j].0[1] - a.0[1]).abs() > 1e-4);
        let (y0, y1) = match mate {
            Some(j) => {
                used[j] = true;
                (a.0[1].min(flats[j].0[1]), a.0[1].max(flats[j].0[1]))
            }
            None => (a.0[1], a.0[1]),
        };
        out.push(([a.0[0], y0, a.0[2]], [a.1[0], y1, a.1[2]]));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// The thrown hat (`HatEntity`)

impl Thrown {
    pub fn to_json(&self) -> Value {
        json!({ "hatPart": self.hat.to_json(), "pos": self.pos, "motion": self.motion,
            "launcherId": self.launcher, "leftLauncher": self.left_launcher, "age": self.age,
            "onGround": self.on_ground, "rot": self.rot, "rotFactor": self.rot_factor })
    }
    pub fn from_json(v: &Value) -> Option<Thrown> {
        let a3 = |k: &str| -> Option<[f64; 3]> {
            let a = v[k].as_array()?;
            Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
        };
        let a2 = |k: &str| -> [f32; 2] {
            let a = v[k].as_array();
            let g = |i: usize| a.and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            [g(0), g(1)]
        };
        let pos = a3("pos")?;
        Some(Thrown {
            hat: HatPart::from_json(&v["hatPart"]),
            pos,
            prev_pos: pos,
            prev_rot: a2("rot"),
            motion: a3("motion").unwrap_or([0.0; 3]),
            launcher: v["launcherId"].as_str().map(str::to_owned),
            left_launcher: v["leftLauncher"].as_bool().unwrap_or(false),
            age: v["age"].as_i64().unwrap_or(0) as i32,
            on_ground: v["onGround"].as_bool().unwrap_or(false),
            rot: a2("rot"),
            rot_factor: a2("rotFactor"),
        })
    }
}

/// What the world answers for a thrown hat.
pub trait HatWorld {
    /// Whether the block at the position is solid for a small entity (has a collision box at that point).
    fn solid(&self, p: [f64; 3]) -> bool;
}

/// One `HatEntity` in flight or on the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct Thrown {
    pub hat: HatPart,
    pub pos: [f64; 3],
    /// Where it was a tick ago (`xo/yo/zo`, `lastRotX/Y`), for drawing between ticks.
    pub prev_pos: [f64; 3],
    pub prev_rot: [f32; 2],
    pub motion: [f64; 3],
    /// The entity that launched it (or last wore it): its UUID string; it cannot be hit until it left 1.5 blocks.
    pub launcher: Option<String>,
    pub left_launcher: bool,
    pub age: i32,
    pub on_ground: bool,
    pub rot: [f32; 2],
    pub rot_factor: [f32; 2],
}

/// `HatEntity.launch`: from the shooter's eyes along its look, at 1.2 blocks a tick.
pub fn launch(hat: HatPart, eye: [f64; 3], look: [f64; 3], shooter: &str, rand: &mut JavaRandom) -> Thrown {
    let pos = [eye[0] + look[0] * 0.5, eye[1] - 0.15, eye[2] + look[2] * 0.5];
    Thrown {
        hat,
        pos,
        prev_pos: pos,
        prev_rot: [0.0; 2],
        motion: [look[0] * 1.2, look[1] * 1.2, look[2] * 1.2],
        launcher: Some(shooter.to_owned()),
        left_launcher: false,
        age: 0,
        on_ground: false,
        rot: [0.0; 2],
        rot_factor: [(rand.next_float() * 2.0 - 1.0) * 45.0, (rand.next_float() * 2.0 - 1.0) * 45.0],
    }
}

/// A living entity a thrown hat can hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub uuid: String,
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub spectator: bool,
}

/// What a tick of a thrown hat did.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Flying,
    /// `discard()` after `hatEntityLifespan` ticks.
    Expired,
    /// It hit this target (index into the list given).
    Hit(usize),
}

/// The hat's box: 0.5 x 0.5 (`EntityType.Builder.sized` of `hat`), centred on x/z, standing on `pos`.
const HALF: f64 = 0.25;

/// `HatEntity.tick` on the server.
pub fn tick_thrown(t: &mut Thrown, world: &impl HatWorld, targets: &[Target]) -> Step {
    t.prev_pos = t.pos;
    t.prev_rot = t.rot;
    t.age += 1;
    if t.age >= config::HAT_ENTITY_LIFESPAN {
        return Step::Expired;
    }
    let motion = t.motion;
    let bb = |p: [f64; 3]| ([p[0] - HALF, p[1], p[2] - HALF], [p[0] + HALF, p[1] + 0.5, p[2] + HALF]);
    let (min, max) = bb(t.pos);
    if !t.left_launcher && t.launcher.is_some() {
        let near = targets.iter().any(|e| {
            Some(&e.uuid) == t.launcher.as_ref()
                && e.max[0] > min[0] - 1.5
                && e.min[0] < max[0] + 1.5
                && e.max[1] > min[1] - 1.5
                && e.min[1] < max[1] + 1.5
                && e.max[2] > min[2] - 1.5
                && e.min[2] < max[2] + 1.5
        });
        if !near {
            t.left_launcher = true;
        }
    }
    if t.left_launcher {
        // getBoundingBox().expandTowards(motion).inflate(0.5)
        let lo: [f64; 3] = std::array::from_fn(|i| min[i] + motion[i].min(0.0) - 0.5);
        let hi: [f64; 3] = std::array::from_fn(|i| max[i] + motion[i].max(0.0) + 0.5);
        if let Some(i) = targets.iter().position(|e| {
            !e.spectator
                && (0..3).all(|k| e.max[k] > lo[k] && e.min[k] < hi[k])
                && !(!t.left_launcher && t.launcher.as_ref() == Some(&e.uuid))
        }) {
            return Step::Hit(i);
        }
    }
    // move(MoverType.SELF, motion): axis by axis (y, then x, then z) against solid blocks.
    let was_ground = t.on_ground;
    let mut pos = t.pos;
    let mut hit_h = false;
    let mut landed = false;
    for axis in [1usize, 0, 2] {
        let mut next = pos;
        next[axis] += motion[axis];
        let (lo, hi) = bb(next);
        let probe = [
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [lo[0], lo[1], hi[2]],
            [hi[0], lo[1], hi[2]],
            [lo[0], hi[1], lo[2]],
            [hi[0], hi[1], hi[2]],
        ];
        if probe.iter().any(|p| world.solid(*p)) {
            if axis == 1 {
                if motion[1] < 0.0 {
                    landed = true;
                    // rest on the block's top
                    pos[1] = (lo[1]).floor() + 1.0;
                    if world.solid([pos[0], pos[1], pos[2]]) {
                        pos[1] = t.pos[1];
                    }
                }
                t.motion[1] = 0.0;
            } else {
                hit_h = true;
                t.motion[axis] = 0.0;
            }
        } else {
            pos = next;
        }
    }
    t.pos = pos;
    t.on_ground = landed || (was_ground && world.solid([pos[0], pos[1] - 0.01, pos[2]]));
    if (hit_h || (t.on_ground && !was_ground)) && t.launcher.is_some() {
        t.left_launcher = true;
    }
    t.motion[1] -= 0.02;
    let drag = if t.on_ground { 0.8 } else { 0.98 };
    for c in &mut t.motion {
        *c *= drag;
    }
    t.rot_factor[0] *= drag as f32;
    t.rot_factor[1] *= drag as f32;
    t.rot[0] += t.rot_factor[0];
    t.rot[1] += t.rot_factor[1];
    if t.on_ground {
        t.motion[1] = t.motion[1].max(0.0);
    }
    Step::Flying
}

/// `HatEntity.hitEntity` on a mob: it wears the incoming hat (showing, with its accessories showing); its old hat
/// bounces off as a new thrown hat (`isRogue`), last touched by that mob. Players get the hat into their collection.
pub fn hit_mob(t: &Thrown, existing: Option<HatPart>, target_uuid: &str, rand: &mut JavaRandom) -> (HatPart, Option<Thrown>) {
    let mut incoming = t.hat.clone();
    incoming.showing = true;
    for a in &mut incoming.accessories {
        a.showing = true;
    }
    let bounced = existing.filter(|h| !h.name.is_empty()).map(|old| Thrown {
        hat: old,
        pos: t.pos,
        prev_pos: t.pos,
        prev_rot: t.rot,
        motion: [gaussian(rand) * 0.2, 0.2 + f64::from(rand.next_float()) * 0.2, gaussian(rand) * 0.2],
        launcher: Some(target_uuid.to_owned()),
        left_launcher: false,
        age: t.age,
        on_ground: false,
        rot: t.rot,
        rot_factor: t.rot_factor,
    });
    (incoming, bounced)
}

/// A normally distributed double (`RandomSource.nextGaussian`, polar method of `java.util.Random`).
fn gaussian(rand: &mut JavaRandom) -> f64 {
    loop {
        let v1 = 2.0 * rand.next_double() - 1.0;
        let v2 = 2.0 * rand.next_double() - 1.0;
        let s = v1 * v1 + v2 * v2;
        if s < 1.0 && s != 0.0 {
            return v1 * (-2.0 * s.ln() / s).sqrt();
        }
    }
}

/// Right-clicking a thrown hat (`HatEntity.interact`): the nearest one the player's look ray passes within
/// `0.25 + 0.3` blocks of (the box inflated by `getPickRadius`), up to `reach`.
pub fn picked(hats: &[Thrown], eye: [f64; 3], look: [f64; 3], reach: f64) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, h) in hats.iter().enumerate() {
        let c = [h.pos[0], h.pos[1] + 0.25, h.pos[2]];
        let d: [f64; 3] = std::array::from_fn(|k| c[k] - eye[k]);
        let along = d[0] * look[0] + d[1] * look[1] + d[2] * look[2];
        if along < 0.0 || along > reach {
            continue;
        }
        let closest: [f64; 3] = std::array::from_fn(|k| eye[k] + look[k] * along);
        let dist2: f64 = (0..3).map(|k| (closest[k] - c[k]).powi(2)).sum();
        if dist2 <= 0.35 * 0.35 && best.is_none_or(|(_, a)| along < a) {
            best = Some((i, along));
        }
    }
    best.map(|(i, _)| i)
}

// ---------------------------------------------------------------------------------------------------------------
// The toast and the menu

/// `HatToast` in a `ToastComponent` slot: 600 ms slide in, shown 5 s (`DISPLAY_TIME_MS`), 600 ms slide out.
/// Returns (portion of the 160 px width shown, finished) at `ms` since it took its slot.
pub fn toast_slide(ms: i64) -> (f32, bool) {
    const SLIDE: i64 = 600;
    const SHOW: i64 = 5000;
    let ease = |t: f32| t * t;
    if ms < SLIDE {
        return (ease(ms as f32 / SLIDE as f32), false);
    }
    if ms < SLIDE + SHOW {
        return (1.0, false);
    }
    let out = ms - SLIDE - SHOW;
    (1.0 - ease((out as f32 / SLIDE as f32).min(1.0)), out >= SLIDE)
}

/// The order of the collection in the menu (`HatsScreen` sort modes: alphabetical, rarity, unlocked, accessories).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Alpha,
    Rarity,
    Unlocked,
    Accessories,
}
impl Sort {
    pub fn next(self) -> Sort {
        match self {
            Sort::Alpha => Sort::Rarity,
            Sort::Rarity => Sort::Unlocked,
            Sort::Unlocked => Sort::Accessories,
            Sort::Accessories => Sort::Alpha,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Sort::Alpha => "A-Z",
            Sort::Rarity => "Rarity",
            Sort::Unlocked => "Unlocked",
            Sort::Accessories => "Accessories",
        }
    }
}

/// The indices of `inventory` shown, filtered by `search` (case-insensitive, on the name) and sorted.
pub fn menu_list(reg: &Registry, inventory: &[HatPart], search: &str, sort: Sort) -> Vec<usize> {
    let s = search.to_lowercase();
    let mut out: Vec<usize> = (0..inventory.len())
        .filter(|&i| inventory[i].count > 0 && (s.is_empty() || inventory[i].name.to_lowercase().contains(&s)))
        .collect();
    let rarity = |i: usize| reg.hat_named(&inventory[i].name).map_or(Rarity::Common, |d| d.rarity);
    match sort {
        Sort::Alpha => out.sort_by(|&a, &b| inventory[a].name.cmp(&inventory[b].name)),
        Sort::Rarity => out.sort_by(|&a, &b| rarity(b).cmp(&rarity(a)).then(inventory[a].name.cmp(&inventory[b].name))),
        Sort::Unlocked => {}
        Sort::Accessories => out.sort_by(|&a, &b| {
            inventory[b].accessories.len().cmp(&inventory[a].accessories.len()).then(inventory[a].name.cmp(&inventory[b].name))
        }),
    }
    // Favourites first, as the grid pins them.
    out.sort_by_key(|&i| !inventory[i].favourite);
    out
}

/// Equipping from the menu: a copy of the owned hat with its accessories' showing flags as the player set them.
pub fn equip_copy(owned: &HatPart) -> HatPart {
    let mut h = owned.clone();
    h.showing = true;
    h.is_new = false;
    h.count = 1;
    h
}

}

/// Used by kit_state for `<world>/data/<MOD_ID>/`.
const MOD_ID: &str = "hats";

// kit_blocks (tools/kit): block names, properties and tags as the finished mods read them
// (fallingtree, comforts, kleeslabs, respawn_anchor.rs on the pin). Pasted by `tools/hnmod.py bundle`
// in place of `include!("kit_blocks.rs");` inside the mod's runtime module `crate::game::<id>`.
// Use: `kit_blocks::name_at(self, pos)`, `kit_blocks::block_in_tag(self, "minecraft:logs", &name)`.
// Tag names are the 26.3 ones (`minecraft:logs`, `c:...`); 1.20 names (`forge:...`) do not exist.
#[allow(dead_code)]
mod kit_blocks {
    use crate::game::Game;
    use horizon_world::registry::BlockStateId;

    /// Inside the world height and in a loaded chunk.
    pub(super) fn loaded(g: &Game<'_>, p: [i32; 3]) -> bool {
        p[1] >= g.dim.min_y() && p[1] <= g.dim.max_y() && g.world.loaded(p[0], p[2])
    }
    /// The state at `p` (air outside the world, as `world.block` answers).
    pub(super) fn state_at(g: &Game<'_>, p: [i32; 3]) -> BlockStateId {
        g.world.block(p[0], p[1], p[2])
    }
    /// The block name of a state (`"minecraft:oak_log"`).
    pub(super) fn name_of(g: &Game<'_>, state: BlockStateId) -> std::sync::Arc<str> {
        g.res.facts().get(state).name.clone()
    }
    /// The block name at `p`; `None` when not loaded or outside the world.
    pub(super) fn name_at(g: &Game<'_>, p: [i32; 3]) -> Option<std::sync::Arc<str>> {
        loaded(g, p).then(|| name_of(g, state_at(g, p)))
    }
    /// A block state property (`facing`, `half`, `age`...).
    pub(super) fn prop(g: &Game<'_>, state: BlockStateId, key: &str) -> Option<String> {
        g.pack
            .assets
            .registry
            .state(state)
            .and_then(|s| s.properties.get(key).cloned())
    }
    /// The same block with some properties changed; `None` if a value is not valid.
    pub(super) fn with_props(
        g: &Game<'_>,
        state: BlockStateId,
        props: &[(&str, &str)],
    ) -> Option<BlockStateId> {
        g.with_props(state, props)
    }
    /// `state.is(BlockTags.X)`: block tag membership by block name.
    pub(super) fn block_in_tag(g: &Game<'_>, tag: &str, block: &str) -> bool {
        g.res
            .items()
            .block_tags
            .get(tag)
            .is_some_and(|s| s.contains(block))
    }
    /// `stack.is(ItemTags.X)`: item tag membership by item name.
    pub(super) fn item_in_tag(g: &Game<'_>, tag: &str, item: &str) -> bool {
        g.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|s| s.contains(item))
    }
    /// The item in the hand that acts (`getItemInHand(hand)`, `Game::held`).
    pub(super) fn held_name(g: &Game<'_>) -> Option<String> {
        g.held().map(|s| s.item.to_string())
    }
    /// The held item is in an item tag (`minecraft:axes`, `minecraft:pickaxes`...).
    pub(super) fn held_in_tag(g: &Game<'_>, tag: &str) -> bool {
        g.held().is_some_and(|s| item_in_tag(g, tag, &s.item))
    }
    /// The player is sneaking (`isShiftKeyDown`), as fallingtree reads it.
    pub(super) fn sneaking(g: &Game<'_>) -> bool {
        g.player.as_ref().is_some_and(|p| p.shift_key_down)
    }
    /// A block of namespace `namespace` (`is_mod_block(name, "mcwwindows")`).
    pub(super) fn is_mod_block(name: &str, namespace: &str) -> bool {
        name.strip_prefix(namespace)
            .is_some_and(|rest| rest.starts_with(':'))
    }
}
// kit_sound (tools/kit): sounds, level events and particles at a block. Pasted by
// `tools/hnmod.py bundle` in place of `include!("kit_sound.rs");`.
// - `audio.play` is captured and sent to LAN guests too (`Audio::play` vs `play_unshared`).
// - `level_event_at` queues `Level.levelEvent` (2001 = block break particles + sound, 1000s =
//   sounds); everyone around sees/hears it.
// - `particles.add` is this window only (guests do not see it).
#[allow(dead_code)]
mod kit_sound {
    use crate::game::Game;

    fn center(p: [i32; 3]) -> [f64; 3] {
        [
            f64::from(p[0]) + 0.5,
            f64::from(p[1]) + 0.5,
            f64::from(p[2]) + 0.5,
        ]
    }
    /// `level.playSound(null, pos, SoundEvents.X, SoundSource.BLOCKS, volume, pitch)`.
    pub(super) fn at_block(g: &mut Game<'_>, sound: &str, pos: [i32; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(center(pos)), volume, pitch);
    }
    /// A sound at an exact point (entities, items).
    pub(super) fn at(g: &mut Game<'_>, sound: &str, at: [f64; 3], volume: f32, pitch: f32) {
        g.audio.play(sound, Some(at), volume, pitch);
    }
    /// `level.levelEvent(id, pos, data)`; `2001` with the block state id = break effect.
    pub(super) fn level_event(g: &mut Game<'_>, id: i32, pos: [i32; 3], data: i32) {
        g.level_event_at(id, pos, data);
    }
    /// `level.addParticle(type, x, y, z, dx, dy, dz)` for this window only.
    pub(super) fn particle(
        g: &mut Game<'_>,
        ty: horizon_client::particles::ParticleType,
        at: [f64; 3],
        velocity: [f64; 3],
    ) {
        let options = horizon_client::particles::ParticleOptions::simple(ty);
        g.particles.add(&options, at, velocity);
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
