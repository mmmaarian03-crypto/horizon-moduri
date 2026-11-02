// Hats Renewed (hats) for Horizon: the rules, without any game code.
// Written from the pack's 1.20.1 jar `HatsRenewed-20.1.2-mc1.20.1-forge.jar` (decompiled in sources/hats/dec; LGPL):
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
