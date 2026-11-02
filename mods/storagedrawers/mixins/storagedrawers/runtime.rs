// Storage Drawers inside Horizon's game (nightly 142388e1c). The rules are the engine below;
// this part is the game's side. A drawer's items are Horizon's own `BlockEntity::Chest(Vec<..>)`
// (the drawer's slots, then 7 upgrade slots), kept in `world.block_entities`, so saving, the
// spill on removal and hoppers work on them (see ironchest/FUNDATIE.md). What is not an item
// (lock, quantify, concealment, suspend, priority, owner) is the engine's `Attrs`, one table for
// the world, written to `data/storagedrawers/drawers.txt` beside the saves.
//
// The hooks (mixins/storagedrawers/horizon-mixin.toml) call the `storagedrawers_*` methods here;
// the game library's hooks use `horizon_client::storagedrawers` (mixins/storagedrawers/shared.rs).
use super::Game;
use horizon_client::{containers::BlockEntity, items::ItemStack};
use horizon_core::position::BlockPos;
use horizon_world::level::flags as level_flags;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use winit::keyboard::KeyCode;

// Parts of the rules (the config values, some helpers) only the tests use.
#[allow(dead_code)]
mod engine {
    // Storage Drawers for Horizon: the rules, without any game code.
// Written from Storage Drawers 1.20.1 (jaquadro, MIT; the jar the pack plays is 12.15.1, the
// source is the `1.20` branch in surse/storagedrawers) and the pack's storagedrawers-common.v2.toml.
// Sources: BlockDrawers, BlockEntityDrawers(+Standard/Comp), StandardDrawerGroup,
// FractionalDrawerGroup, UpgradeData, CompTierRegistry, BlockController, ItemKey classes.
//
// The game side (native/horizon.rs) keeps the items of a drawer in Horizon's own
// `BlockEntity::Chest(Vec<..>)` (drawer slots first, then the 7 upgrade slots), so saving, spills
// on removal and hoppers work on them. What is not an item (lock, quantify, concealment, suspend,
// priority, owner, the item a locked empty slot remembers) is `Attrs`, kept per position by the
// game side and written in the world's own file (`State`).

pub type Pos = [i32; 3];

/// `DrawerUpgradeData` is built with 7 slots.
pub const UPGRADE_SLOTS: usize = 7;
/// Counts are Java ints: `Integer.MAX_VALUE` is the creative / saturation limit.
pub const MAX_COUNT: u32 = i32::MAX as u32;
/// `BlockEntityDrawers.interactPutItemsIntoSlot`: a second click within 10 ticks of the same
/// player's first puts the whole inventory.
pub const DOUBLE_CLICK_TICKS: i64 = 10;

/// The values of the mod's config file (the pack's `storagedrawers-common.v2.toml`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// `Drawers.baseStackStorage`.
    pub base_stack_storage: u32,
    /// `Drawers.Blocks.*.unitsPerSlot`: full 1x1, 1x2, 2x2, compacting.
    pub full: [u32; 4],
    /// The same for half drawers.
    pub half: [u32; 4],
    /// `Upgrades.StorageTiers.*.storageMultiplier` by level 0..=7 (level 0 is the default of 1).
    pub storage_mult: [u32; 8],
    /// `Controller.controllerRange`.
    pub controller_range: i32,
    /// `Drawers.Compacting.enableExtraCompactingRules`.
    pub extra_compacting_rules: bool,
    /// `Drawers.Storage.storeDenyList`.
    pub store_deny: Vec<String>,
    /// `Drawers.Compacting.compactingRules`: upper, lower, rate.
    pub compacting_rules: Vec<(String, String, u32)>,
}
impl Config {
    pub fn tuned() -> Config {
        Config {
            base_stack_storage: 1,
            full: [32, 16, 8, 32],
            half: [16, 8, 4, 16],
            storage_mult: [1, 2, 4, 8, 16, 64, 256, 2048],
            controller_range: 50,
            extra_compacting_rules: true,
            store_deny: vec!["storagedrawers:creative_vending_upgrade".to_owned()],
            compacting_rules: vec![(
                "minecraft:clay".to_owned(),
                "minecraft:clay_ball".to_owned(),
                4,
            )],
        }
    }
    /// `StorageBlacklist`: an entry is an item id or a namespace.
    pub fn denied(&self, item: &str) -> bool {
        self.store_deny.iter().any(|d| {
            d == item || (!d.contains(':') && item.split(':').next() == Some(d.as_str()))
        })
    }
}

// ---------------------------------------------------------------- blocks

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// 1, 2 or 4 independent slots.
    Standard(usize),
    /// 2 or 3 tiers of one item sharing a pool (`FractionalDrawerGroup`).
    Compacting(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub layout: Layout,
    pub half: bool,
    pub framed: bool,
    /// `unitsPerSlot` from the config.
    pub units: u32,
}
impl Block {
    /// Slots used in the block entity before the upgrade slots.
    pub fn drawers(&self) -> usize {
        match self.layout {
            Layout::Standard(n) | Layout::Compacting(n) => n,
        }
    }
    pub fn chest_size(&self) -> usize {
        self.drawers() + UPGRADE_SLOTS
    }
    pub fn compacting(&self) -> bool {
        matches!(self.layout, Layout::Compacting(_))
    }
}

/// What a block of the mod is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Drawer(Block),
    Controller,
    /// `controller_slave` / `controller_io` / trims: networked, no behaviour of their own.
    Networked,
    Other,
}

const WOODS: [&str; 11] = [
    "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "bamboo",
    "crimson", "warped",
];

pub fn kind(name: &str, cfg: &Config) -> Kind {
    let Some(path) = name.strip_prefix("storagedrawers:") else {
        return Kind::Other;
    };
    match path {
        "controller" | "framed_controller" => return Kind::Controller,
        "controller_io" | "controller_slave" | "framed_controller_io" | "framed_trim" => {
            return Kind::Networked;
        }
        _ => {}
    }
    if let Some(w) = path.strip_suffix("_trim")
        && WOODS.contains(&w)
    {
        return Kind::Networked;
    }
    let (framed, rest) = match path.strip_prefix("framed_") {
        Some(r) => (true, r),
        None => (false, path),
    };
    if let Some(r) = rest.strip_prefix("compacting_") {
        let (half, r) = match r.strip_prefix("half_") {
            Some(r) => (true, r),
            None => (false, r),
        };
        let tiers = match r {
            "drawers_2" => 2,
            "drawers_3" => 3,
            _ => return Kind::Other,
        };
        let units = if half { cfg.half[3] } else { cfg.full[3] };
        return Kind::Drawer(Block {
            layout: Layout::Compacting(tiers),
            half,
            framed,
            units,
        });
    }
    // <wood>_<full|half>_drawers_<n>, or framed_<full|half>_drawers_<n>
    let rest = if framed {
        rest
    } else {
        let mut found = None;
        for w in WOODS {
            if let Some(r) = rest.strip_prefix(w).and_then(|r| r.strip_prefix('_')) {
                found = Some(r);
                break;
            }
        }
        match found {
            Some(r) => r,
            None => return Kind::Other,
        }
    };
    let (half, r) = if let Some(r) = rest.strip_prefix("full_") {
        (false, r)
    } else if let Some(r) = rest.strip_prefix("half_") {
        (true, r)
    } else {
        return Kind::Other;
    };
    let (slots, idx) = match r {
        "drawers_1" => (1, 0),
        "drawers_2" => (2, 1),
        "drawers_4" => (4, 2),
        _ => return Kind::Other,
    };
    let units = if half { cfg.half[idx] } else { cfg.full[idx] };
    Kind::Drawer(Block {
        layout: Layout::Standard(slots),
        half,
        framed,
        units,
    })
}

/// The size of the block entity of a block of ours, if it has one.
pub fn chest_size(name: &str, cfg: &Config) -> Option<usize> {
    match kind(name, cfg) {
        Kind::Drawer(b) => Some(b.chest_size()),
        _ => None,
    }
}

// ---------------------------------------------------------------- upgrades

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upgrade {
    Storage(u8),
    OneStack,
    Void,
    CreativeStorage,
    CreativeVending,
    Conversion,
    Redstone,
    MinRedstone,
    MaxRedstone,
    Illumination,
    FillLevel,
    Balance,
    Portability,
    Hopper,
    Magnet(u8),
    Push(u8),
    Pull(u8),
    Remote,
    RemoteGroup,
}
impl Upgrade {
    /// The upgrade an item id is, if any.
    pub fn parse(item: &str) -> Option<Upgrade> {
        let p = item.strip_prefix("storagedrawers:")?;
        let p = p.strip_suffix("_bound").unwrap_or(p);
        Some(match p {
            "obsidian_storage_upgrade" => Upgrade::Storage(1),
            "copper_storage_upgrade" => Upgrade::Storage(2),
            "iron_storage_upgrade" => Upgrade::Storage(3),
            "gold_storage_upgrade" => Upgrade::Storage(4),
            "emerald_storage_upgrade" => Upgrade::Storage(5),
            "diamond_storage_upgrade" => Upgrade::Storage(6),
            "netherite_storage_upgrade" => Upgrade::Storage(7),
            "one_stack_upgrade" => Upgrade::OneStack,
            "void_upgrade" => Upgrade::Void,
            "creative_storage_upgrade" => Upgrade::CreativeStorage,
            "creative_vending_upgrade" => Upgrade::CreativeVending,
            "conversion_upgrade" => Upgrade::Conversion,
            "redstone_upgrade" => Upgrade::Redstone,
            "min_redstone_upgrade" => Upgrade::MinRedstone,
            "max_redstone_upgrade" => Upgrade::MaxRedstone,
            "illumination_upgrade" => Upgrade::Illumination,
            "fill_level_upgrade" => Upgrade::FillLevel,
            "balance_fill_upgrade" => Upgrade::Balance,
            "portability_upgrade" => Upgrade::Portability,
            "hopper_upgrade" => Upgrade::Hopper,
            "magnet_upgrade" => Upgrade::Magnet(1),
            "magnet_upgrade_2" => Upgrade::Magnet(2),
            "magnet_upgrade_3" => Upgrade::Magnet(3),
            "push_upgrade" => Upgrade::Push(1),
            "push_upgrade_2" => Upgrade::Push(2),
            "push_upgrade_3" => Upgrade::Push(3),
            "pull_upgrade" => Upgrade::Pull(1),
            "pull_upgrade_2" => Upgrade::Pull(2),
            "pull_upgrade_3" => Upgrade::Pull(3),
            "remote_upgrade" => Upgrade::Remote,
            "remote_group_upgrade" => Upgrade::RemoteGroup,
            _ => return None,
        })
    }
    /// `ItemUpgrade.getUpgradeGroup`: storage tiers share one, push levels one, pull levels one,
    /// every other upgrade has its own.
    pub fn group(self) -> u32 {
        match self {
            Upgrade::Storage(_) => 0,
            Upgrade::Push(_) => 1,
            Upgrade::Pull(_) => 2,
            Upgrade::OneStack => 3,
            Upgrade::Void => 4,
            Upgrade::CreativeStorage => 5,
            Upgrade::CreativeVending => 6,
            Upgrade::Conversion => 7,
            Upgrade::Redstone => 8,
            Upgrade::MinRedstone => 9,
            Upgrade::MaxRedstone => 10,
            Upgrade::Illumination => 11,
            Upgrade::FillLevel => 12,
            Upgrade::Balance => 13,
            Upgrade::Portability => 14,
            Upgrade::Hopper => 15,
            Upgrade::Magnet(_) => 16,
            Upgrade::Remote => 17,
            Upgrade::RemoteGroup => 18,
        }
    }
    /// `setAllowMultiple(true)`: storage tiers and magnets (push/pull set false).
    pub fn allow_multiple(self) -> bool {
        matches!(self, Upgrade::Storage(_) | Upgrade::Magnet(_))
    }
}

/// The upgrades in the 7 slots (`None` = empty), by item id.
pub fn upgrades_of(items: &[Option<&str>]) -> Vec<Upgrade> {
    items
        .iter()
        .flatten()
        .filter_map(|i| Upgrade::parse(i))
        .collect()
}

/// `UpgradeData.canAddUpgrade`: not the same group twice unless the upgrade allows several.
pub fn can_add_upgrade(present: &[Upgrade], new: Upgrade) -> bool {
    new.allow_multiple() || !present.iter().any(|u| u.group() == new.group())
}

/// `UpgradeData.syncStorageMultiplier`: the multipliers add; none gives the level-0 value.
pub fn storage_multiplier(ups: &[Upgrade], cfg: &Config) -> u32 {
    let sum: u64 = ups
        .iter()
        .filter_map(|u| match u {
            Upgrade::Storage(l) => Some(u64::from(cfg.storage_mult[usize::from(*l).min(7)])),
            _ => None,
        })
        .sum();
    if sum == 0 {
        cfg.storage_mult[0]
    } else {
        sum.min(u64::from(MAX_COUNT)) as u32
    }
}

pub fn has(ups: &[Upgrade], wanted: Upgrade) -> bool {
    ups.contains(&wanted)
}
pub fn is_void(ups: &[Upgrade]) -> bool {
    has(ups, Upgrade::Void)
}
pub fn is_unlimited(ups: &[Upgrade]) -> bool {
    has(ups, Upgrade::CreativeStorage) || has(ups, Upgrade::CreativeVending)
}
pub fn is_vending(ups: &[Upgrade]) -> bool {
    has(ups, Upgrade::CreativeVending)
}

/// `getEffectiveDrawerCapacity * getStorageMultiplier`: the stacks a slot holds
/// (`getStackCapacity`).
pub fn stack_capacity(block: &Block, ups: &[Upgrade], cfg: &Config) -> u32 {
    let effective: u64 = if has(ups, Upgrade::OneStack) {
        1
    } else {
        u64::from(block.units) * u64::from(cfg.base_stack_storage)
    };
    (effective * u64::from(storage_multiplier(ups, cfg))).min(u64::from(MAX_COUNT)) as u32
}

/// `DrawerData.getMaxCapacity(proto)`: `getStackSize * getStackCapacity`, Integer.MAX_VALUE for
/// the creative upgrades and on overflow.
pub fn slot_capacity(block: &Block, ups: &[Upgrade], cfg: &Config, max_stack: u32) -> u32 {
    if is_unlimited(ups) {
        return MAX_COUNT;
    }
    (u64::from(max_stack.max(1)) * u64::from(stack_capacity(block, ups, cfg)))
        .min(u64::from(MAX_COUNT)) as u32
}

/// Insert into one slot: how many items leave the offered stack and how many are stored. Void
/// drawers swallow the surplus; vending drawers swallow everything
/// (`BlockEntityDrawers.putItemsIntoSlot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insert {
    pub consumed: u32,
    pub stored: u32,
}
pub fn insert(count: u32, cap: u32, offered: u32, ups: &[Upgrade]) -> Insert {
    if is_vending(ups) {
        return Insert {
            consumed: offered,
            stored: 0,
        };
    }
    let room = cap.saturating_sub(count);
    let stored = offered.min(room);
    let consumed = if is_void(ups) { offered } else { stored };
    Insert { consumed, stored }
}
/// `takeItemsFromSlot`: items taken, and the count left (vending drawers keep theirs).
pub fn extract(count: u32, wanted: u32, ups: &[Upgrade]) -> (u32, u32) {
    if is_vending(ups) {
        return (wanted.min(MAX_COUNT), count);
    }
    let taken = wanted.min(count);
    (taken, count - taken)
}

/// `DrawerData.canItemBeStored`: `populated` = the slot has an item (or remembers one when
/// locked), `same` = the offered item is that item. `manual` = a player's click (a locked empty
/// slot still takes anything from a player).
pub fn can_store(populated: bool, same: bool, locked: bool, manual: bool) -> bool {
    if !populated {
        return manual || !locked;
    }
    same
}

// ---------------------------------------------------------------- compacting

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub upper: String,
    pub lower: String,
    pub rate: u32,
}
fn rule(upper: String, lower: String, rate: u32) -> Rule {
    Rule { upper, lower, rate }
}
/// The tiers `CompactingHelper` finds in the crafting recipes (a 2x2 or 3x3 recipe whose single
/// result crafts back into the same count), written out: the game side has no recipe search for
/// it. `CompTierRegistry`'s extra rules and the config's come first (they win over recipes).
pub fn rules(cfg: &Config) -> Vec<Rule> {
    let mut v: Vec<Rule> = Vec::new();
    // CompTierRegistry.initialize (enableExtraCompactingRules).
    if cfg.extra_compacting_rules {
        for (u, l, r) in [
            ("clay", "clay_ball", 4),
            ("snow_block", "snowball", 4),
            ("glowstone", "glowstone_dust", 4),
            ("bricks", "brick", 4),
            ("nether_bricks", "nether_brick", 4),
            ("nether_wart_block", "nether_wart", 9),
            ("quartz_block", "quartz", 4),
            ("melon", "melon_slice", 9),
            ("bamboo_block", "bamboo", 9),
        ] {
            v.push(rule(format!("minecraft:{u}"), format!("minecraft:{l}"), r));
        }
    }
    for (u, l, r) in &cfg.compacting_rules {
        v.retain(|x| x.upper != *u && x.lower != *l);
        v.push(rule(u.clone(), l.clone(), *r));
    }
    // Recipe-derived (vanilla storage blocks, nugget/ingot pairs, slime, hay, bone, resin).
    for (u, l) in [
        ("iron_block", "iron_ingot"),
        ("gold_block", "gold_ingot"),
        ("copper_block", "copper_ingot"),
        ("netherite_block", "netherite_ingot"),
        ("diamond_block", "diamond"),
        ("emerald_block", "emerald"),
        ("lapis_block", "lapis_lazuli"),
        ("redstone_block", "redstone"),
        ("coal_block", "coal"),
        ("raw_iron_block", "raw_iron"),
        ("raw_gold_block", "raw_gold"),
        ("raw_copper_block", "raw_copper"),
        ("hay_block", "wheat"),
        ("bone_block", "bone_meal"),
        ("dried_kelp_block", "dried_kelp"),
        ("slime_block", "slime_ball"),
        ("resin_block", "resin_clump"),
        ("iron_ingot", "iron_nugget"),
        ("gold_ingot", "gold_nugget"),
        ("copper_ingot", "copper_nugget"),
    ] {
        let (u, l) = (format!("minecraft:{u}"), format!("minecraft:{l}"));
        if !v.iter().any(|x| x.upper == u || x.lower == l) {
            v.push(rule(u, l, 9));
        }
    }
    v
}

/// The tiers of a compacting drawer, largest first; `rates[i]` is how many of the smallest item
/// one item of tier `i` is (the smallest has 1). `populateSlots` + `normalizeGroup`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
    pub items: Vec<String>,
    pub rates: Vec<u32>,
}
impl Chain {
    pub fn smallest(&self) -> usize {
        self.items.len() - 1
    }
    pub fn tier_of(&self, item: &str) -> Option<usize> {
        self.items.iter().position(|i| i == item)
    }
}
/// The tiers around `item` for a drawer of `slots` tiers: up to `slots - 1` higher ones, the
/// item, then lower ones while the slots last.
pub fn chain_around(item: &str, slots: usize, rules: &[Rule]) -> Chain {
    let mut ups: Vec<(&str, u32)> = Vec::new(); // (upper item, items of it per one below)
    let mut cur = item;
    for _ in 0..slots.saturating_sub(1) {
        let Some(r) = rules.iter().find(|r| r.lower == cur) else {
            break;
        };
        ups.push((&r.upper, r.rate));
        cur = &r.upper;
    }
    let mut items: Vec<String> = ups.iter().rev().map(|u| u.0.to_owned()).collect();
    // conv[i]: how many of tier i+1 make one of tier i.
    let mut conv: Vec<u32> = ups.iter().rev().map(|u| u.1).collect();
    items.push(item.to_owned());
    let mut cur = item;
    while items.len() < slots {
        let Some(r) = rules.iter().find(|r| r.upper == cur) else {
            break;
        };
        items.push(r.lower.clone());
        conv.push(r.rate);
        cur = &r.lower;
    }
    let mut rates = vec![1u32; items.len()];
    for i in (0..items.len().saturating_sub(1)).rev() {
        rates[i] = rates[i + 1].saturating_mul(conv[i]);
    }
    Chain { items, rates }
}
/// The chain a compacting drawer whose smallest unit is `smallest` has.
pub fn chain_from_smallest(smallest: &str, slots: usize, rules: &[Rule]) -> Chain {
    let mut top = smallest;
    for _ in 0..slots.saturating_sub(1) {
        let Some(r) = rules.iter().find(|r| r.lower == top) else {
            break;
        };
        top = &r.upper;
    }
    let full = chain_around(top, slots, rules);
    if full.items.last().map(String::as_str) == Some(smallest) {
        return full;
    }
    chain_around(smallest, slots, rules)
}
/// `getMaxCapacity(0) * convRate[0]`: the pool's limit, in smallest units.
pub fn pool_capacity(
    chain: &Chain,
    ups: &[Upgrade],
    block: &Block,
    cfg: &Config,
    top_max_stack: u32,
) -> u32 {
    if is_unlimited(ups) {
        return MAX_COUNT;
    }
    (u64::from(top_max_stack.max(1))
        * u64::from(stack_capacity(block, ups, cfg))
        * u64::from(chain.rates[0]))
    .min(u64::from(MAX_COUNT)) as u32
}
/// Add `n` items worth `rate` smallest units each: (items that leave the offered stack, new pool).
pub fn pool_insert(pool: u32, cap: u32, rate: u32, n: u32, ups: &[Upgrade]) -> (u32, u32) {
    let rate = rate.max(1);
    if is_vending(ups) {
        return (n, pool);
    }
    let can_add = cap.saturating_sub(pool) / rate;
    let will = n.min(can_add);
    let consumed = if is_void(ups) { n } else { will };
    (consumed, pool + will * rate)
}
/// Take up to `wanted` items worth `rate` each out: (taken, new pool).
pub fn pool_take(pool: u32, rate: u32, wanted: u32, ups: &[Upgrade]) -> (u32, u32) {
    let rate = rate.max(1);
    if is_vending(ups) {
        return (wanted, pool);
    }
    let taken = wanted.min(pool / rate);
    (taken, pool - taken * rate)
}

// ---------------------------------------------------------------- the network

/// What the controller's search sees at a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    Drawer,
    Other,
    None,
}
/// `BlockEntityController.populateRoot`: the drawers reached from `root` through adjacent
/// networked blocks, no farther than `range` on any axis, in discovery order.
pub fn network(root: Pos, range: i32, node: impl Fn(Pos) -> Node) -> Vec<Pos> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    seen.insert(root);
    queue.push_back(root);
    while let Some(p) = queue.pop_front() {
        let depth = (0..3).map(|i| (p[i] - root[i]).abs()).max().unwrap_or(0);
        if depth > range {
            continue;
        }
        let n = if p == root { Node::Other } else { node(p) };
        if n == Node::None {
            continue;
        }
        if n == Node::Drawer {
            found.push(p);
        }
        for d in [
            [-1, 0, 0],
            [1, 0, 0],
            [0, 0, -1],
            [0, 0, 1],
            [0, 1, 0],
            [0, -1, 0],
        ] {
            let q = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            if seen.insert(q) {
                queue.push_back(q);
            }
        }
    }
    found
}

// ---------------------------------------------------------------- attributes and their file

/// The state of a drawer that is not an item (`BasicDrawerAttributes`, the owner).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attrs {
    /// Both lock attributes at once (the drawer key sets and clears them together).
    pub locked: bool,
    pub quantify: bool,
    pub concealed: bool,
    pub suspended: bool,
    pub priority: i32,
    pub owner: Option<String>,
    /// What each drawer slot remembers when it is locked and empty (item ids; "" = nothing).
    pub protos: Vec<String>,
}
impl Attrs {
    pub fn is_default(&self) -> bool {
        *self == Attrs::default()
    }
    pub fn proto(&self, slot: usize) -> Option<&str> {
        self.protos
            .get(slot)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }
    pub fn set_proto(&mut self, slot: usize, item: Option<&str>) {
        if self.protos.len() <= slot {
            self.protos.resize(slot + 1, String::new());
        }
        self.protos[slot] = item.unwrap_or("").to_owned();
        while self.protos.last().is_some_and(String::is_empty) {
            self.protos.pop();
        }
    }
    /// `ItemPriorityKey.handleDrawerAttributes` for the plain key: cycles 0, 1, 2, -1, -2, 0
    /// (the keys chain p1 -> p2 -> n1 -> n2 -> plain, from `ModItems`).
    pub fn cycle_priority(&mut self) {
        self.priority = match self.priority {
            0 => 1,
            1 => 2,
            2 => -1,
            -1 => -2,
            _ => 0,
        };
    }
    pub fn encode(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        format!(
            "L{}Q{}C{}S{}P{};{};{}",
            b(self.locked),
            b(self.quantify),
            b(self.concealed),
            b(self.suspended),
            self.priority,
            self.owner.as_deref().unwrap_or("-"),
            self.protos.join(",")
        )
    }
    pub fn decode(s: &str) -> Option<Attrs> {
        let mut it = s.splitn(3, ';');
        let flags = it.next()?;
        let owner = it.next()?;
        let protos = it.next().unwrap_or("");
        let bit = |c: char| -> Option<bool> {
            let at = flags.find(c)?;
            Some(flags[at + 1..].starts_with('1'))
        };
        let priority = flags[flags.find('P')? + 1..].parse::<i32>().ok()?;
        Some(Attrs {
            locked: bit('L')?,
            quantify: bit('Q')?,
            concealed: bit('C')?,
            suspended: bit('S')?,
            priority,
            owner: (owner != "-").then(|| owner.to_owned()),
            protos: if protos.is_empty() {
                Vec::new()
            } else {
                protos.split(',').map(str::to_owned).collect()
            },
        })
    }
}

/// The world's drawers: `(dimension, position)` -> attributes, as the file
/// `data/storagedrawers/drawers.txt` (one line each: dimension, x, y, z, attributes, tab separated).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub drawers: std::collections::BTreeMap<(String, Pos), Attrs>,
}
impl State {
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for ((dim, p), a) in &self.drawers {
            if a.is_default() {
                continue;
            }
            out.push_str(&format!(
                "{dim}\t{}\t{}\t{}\t{}\n",
                p[0],
                p[1],
                p[2],
                a.encode()
            ));
        }
        out
    }
    /// Lines that do not parse are skipped.
    pub fn from_text(text: &str) -> State {
        let mut s = State::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 5 {
                continue;
            }
            let (Ok(x), Ok(y), Ok(z)) = (f[1].parse(), f[2].parse(), f[3].parse()) else {
                continue;
            };
            if let Some(a) = Attrs::decode(f[4]) {
                s.drawers.insert((f[0].to_owned(), [x, y, z]), a);
            }
        }
        s
    }
}

// ---------------------------------------------------------------- keys

/// The tools of the mod that act on a drawer (`ItemKey` and kin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Drawer,
    Quantify,
    Shroud,
    Suspend,
    Priority(i32),
    PriorityCycle,
    Personal,
    PersonalUnlock,
    Puller,
}
impl Key {
    /// A key or a keyring holding one (`keyring_<x>` acts as the key `<x>`).
    pub fn parse(item: &str) -> Option<Key> {
        let p = item.strip_prefix("storagedrawers:")?;
        let p = p.strip_prefix("keyring_").unwrap_or(p);
        Some(match p {
            "drawer_key" | "drawer" => Key::Drawer,
            "quantify_key" | "quantify" => Key::Quantify,
            "shroud_key" | "shroud" => Key::Shroud,
            "suspend_key" | "suspend" => Key::Suspend,
            "priority_key" | "priority" => Key::PriorityCycle,
            "priority_key_p1" | "priority_p1" => Key::Priority(1),
            "priority_key_p2" | "priority_p2" => Key::Priority(2),
            "priority_key_n1" | "priority_n1" => Key::Priority(-1),
            "priority_key_n2" | "priority_n2" => Key::Priority(-2),
            "personal_key" | "personal" => Key::Personal,
            "personal_key_unlock" | "personal_unlock" => Key::PersonalUnlock,
            "drawer_puller" | "puller" => Key::Puller,
            _ => return None,
        })
    }
    /// Applies the key to a drawer's attributes; false when it does nothing here.
    pub fn apply(self, a: &mut Attrs, player: &str) -> bool {
        match self {
            Key::Drawer => a.locked = !a.locked,
            Key::Quantify => a.quantify = !a.quantify,
            Key::Shroud => a.concealed = !a.concealed,
            Key::Suspend => a.suspended = !a.suspended,
            Key::PriorityCycle => a.cycle_priority(),
            Key::Priority(l) => a.priority = if a.priority == l { 0 } else { l },
            Key::Personal => match &a.owner {
                None => a.owner = Some(player.to_owned()),
                Some(o) if o == player => a.owner = None,
                Some(_) => return false,
            },
            Key::PersonalUnlock => a.owner = None,
            Key::Puller => return false,
        }
        true
    }
}
/// `SecurityManager.hasAccess`: an unowned drawer is open to everybody.
pub fn has_access(a: &Attrs, player: &str) -> bool {
    a.owner.as_deref().is_none_or(|o| o == player)
}

// ---------------------------------------------------------------- the face

/// `getFaceSlot`: which slot of a drawer's front face a hit is on, as fractions of the face with
/// `u` to the right as seen from the front and `v` up; `None` for the border strip
/// (`BlockStandardDrawers`, `BlockCompDrawers`).
pub fn face_slot(layout: Layout, u: f64, v: f64) -> Option<usize> {
    // hitWithinArea(.0625, .9375)
    if u <= 0.0625 || u >= 0.9375 || v <= 0.0625 || v >= 0.9375 {
        return None;
    }
    let top = v > 0.5;
    let left = u < 0.5;
    match layout {
        Layout::Standard(1) => Some(0),
        Layout::Standard(2) => Some(if top { 0 } else { 1 }),
        Layout::Standard(4) => Some(match (top, left) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        }),
        Layout::Standard(_) => None,
        Layout::Compacting(2) => Some(if top { 0 } else { 1 }),
        Layout::Compacting(3) => Some(if top {
            0
        } else if left {
            1
        } else {
            2
        }),
        Layout::Compacting(_) => None,
    }
}

}
use engine::{Attrs, Block, Chain, Kind, Layout, Upgrade};

fn cfg() -> &'static engine::Config {
    static C: OnceLock<engine::Config> = OnceLock::new();
    C.get_or_init(engine::Config::tuned)
}
fn rules() -> &'static [engine::Rule] {
    static R: OnceLock<Vec<engine::Rule>> = OnceLock::new();
    R.get_or_init(|| engine::rules(cfg()))
}

/// The attributes of every drawer of the world (the engine's `State`).
static STATE: Mutex<Option<engine::State>> = Mutex::new(None);
/// The last click on a drawer: dimension, position, game time, player (the double click).
static CLICK: Mutex<Option<(String, [i32; 3], i64, String)>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut engine::State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(engine::State::default))
}

/// The chest entity key of a position.
fn key_of(p: [i32; 3]) -> (i32, i32, i32) {
    (p[0], p[1], p[2])
}
/// The block's front as a block-state value, `Direction.getOpposite` of the placer's look
/// (`BlockDrawers.getStateForPlacement`): 0 south, 1 west, 2 north, 3 east look -> opposite.
fn placing_facing(yaw: f32) -> &'static str {
    let k = ((yaw / 90.0 + 0.5).floor() as i32) & 3;
    ["north", "east", "south", "west"][k as usize]
}
fn facing_normal(facing: &str) -> Option<[i32; 3]> {
    Some(match facing {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    })
}

/// What a drawer is, with its upgrades and attributes (read before an operation).
struct Drawer {
    block: Block,
    ups: Vec<Upgrade>,
    attrs: Attrs,
}

/// A stack of the mod's own drawers that holds something (`canStoreInDrawers = false`).
fn is_filled_drawer(stack: &ItemStack) -> bool {
    stack.item.starts_with("storagedrawers:")
        && !matches!(engine::kind(&stack.item, cfg()), Kind::Other)
        && stack
            .data()
            .and_then(|d| d.other(horizon_client::block_entity_item::BLOCK_ENTITY_DATA))
            .is_some()
}

impl Game<'_> {
    // ---- reading the world ----

    fn sd_kind_at(&self, p: [i32; 3]) -> Kind {
        let state = self.world.block(p[0], p[1], p[2]);
        let name = self.res.facts().get(state).name.clone();
        engine::kind(&name, cfg())
    }
    fn sd_dim(&self) -> String {
        self.dim.id().to_owned()
    }
    fn sd_attrs(&self, p: [i32; 3]) -> Attrs {
        let dim = self.sd_dim();
        with_state(|s| s.drawers.get(&(dim, p)).cloned().unwrap_or_default())
    }
    fn sd_store_attrs(&self, p: [i32; 3], attrs: Attrs) {
        let dim = self.sd_dim();
        with_state(|s| {
            if attrs.is_default() {
                s.drawers.remove(&(dim, p));
            } else {
                s.drawers.insert((dim, p), attrs);
            }
        });
    }
    fn sd_max_stack(&self, item: &str) -> u32 {
        self.res.items().get(item).map_or(64, |i| i.max_stack).max(1)
    }
    fn sd_facing_at(&self, p: [i32; 3]) -> Option<&'static str> {
        let state = self.world.block(p[0], p[1], p[2]);
        let props = self.pack.assets.registry.state(state).map(|s| &s.properties);
        match props.and_then(|p| p.get("facing")).map(String::as_str) {
            Some("north") => Some("north"),
            Some("south") => Some("south"),
            Some("west") => Some("west"),
            Some("east") => Some("east"),
            _ => None,
        }
    }
    fn sd_player_text(&mut self) -> String {
        horizon_client::save::uuid_text(self.acting_player_uuid())
    }
    fn sd_say(&mut self, text: &str) {
        self.message = Some((text.to_owned(), Instant::now()));
    }

    /// The block entity of the drawer at `p`, made (or grown) to the block's size.
    fn sd_ensure(&mut self, p: [i32; 3], block: &Block) {
        let key = key_of(p);
        let size = block.chest_size();
        match self.world.block_entities.get_mut(&key) {
            Some(BlockEntity::Chest(items)) => {
                if items.len() < size {
                    items.resize(size, None);
                } else if items.len() > size && items[size..].iter().all(Option::is_none) {
                    items.truncate(size);
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
    fn sd_items(&mut self, p: [i32; 3]) -> Option<&mut Vec<Option<ItemStack>>> {
        match self.world.block_entities.get_mut(&key_of(p)) {
            Some(BlockEntity::Chest(items)) => Some(items),
            _ => None,
        }
    }
    fn sd_changed(&mut self, p: [i32; 3]) {
        self.world.modified.insert((p[0] >> 4, p[2] >> 4));
    }
    /// The compacting drawer's pool lives in the smallest tier's cell: move it there.
    fn sd_normalize(&mut self, p: [i32; 3], block: &Block) {
        if !block.compacting() {
            return;
        }
        let n = block.drawers();
        let Some(items) = self.sd_items(p) else {
            return;
        };
        let Some(at) = (0..n).find(|&i| items[i].is_some()) else {
            return;
        };
        let item = items[at].as_ref().map(|s| s.item.to_string()).unwrap_or_default();
        let chain = engine::chain_from_smallest(&item, n, rules());
        let want = chain.smallest();
        if at != want && items[want].is_none() {
            items[want] = items[at].take();
        }
    }
    /// The drawer at `p` as the rules see it (making its block entity on the way).
    fn sd_drawer(&mut self, p: [i32; 3]) -> Option<Drawer> {
        let Kind::Drawer(block) = self.sd_kind_at(p) else {
            return None;
        };
        self.sd_ensure(p, &block);
        self.sd_normalize(p, &block);
        let attrs = self.sd_attrs(p);
        let items = self.sd_items(p)?;
        let ids: Vec<Option<&str>> = items[block.drawers()..]
            .iter()
            .map(|s| s.as_ref().map(|s| &*s.item))
            .collect();
        let ups = engine::upgrades_of(&ids);
        Some(Drawer { block, ups, attrs })
    }
    /// The compacting drawer's tiers and pool, if it holds anything (or is locked on an item).
    fn sd_chain(&mut self, p: [i32; 3], d: &Drawer) -> Option<(Chain, u32)> {
        let n = d.block.drawers();
        let items = self.sd_items(p)?;
        for s in items[..n].iter().flatten() {
            let chain = engine::chain_from_smallest(&s.item, n, rules());
            return Some((chain, s.count));
        }
        if d.attrs.locked
            && let Some(i) = d.attrs.proto(0)
        {
            return Some((engine::chain_from_smallest(i, n, rules()), 0));
        }
        None
    }
    fn sd_write_pool(&mut self, p: [i32; 3], d: &mut Drawer, chain: &Chain, pool: u32) {
        let n = d.block.drawers();
        let small = chain.smallest();
        let item = chain.items[small].clone();
        if let Some(items) = self.sd_items(p) {
            for cell in items[..n].iter_mut() {
                *cell = None;
            }
            // Creative vending keeps its drawer populated even at nothing.
            let keep = pool > 0 || engine::is_vending(&d.ups);
            if keep {
                items[small] = Some(ItemStack::new(&item, pool.max(1)));
            }
        }
        if d.attrs.locked {
            d.attrs.set_proto(0, Some(&item));
        }
        self.sd_changed(p);
    }

    // ---- putting items in ----

    /// Put the stack into slot `slot` of the drawer at `p` (a player's click when `manual`);
    /// the number of items that left the stack. The caller stores `d.attrs`.
    fn sd_put(
        &mut self,
        p: [i32; 3],
        d: &mut Drawer,
        slot: usize,
        stack: &mut ItemStack,
        manual: bool,
    ) -> u32 {
        if slot >= d.block.drawers()
            || stack.count == 0
            || cfg().denied(&stack.item)
            || is_filled_drawer(stack)
        {
            return 0;
        }
        let max = self.sd_max_stack(&stack.item);
        let consumed = match d.block.layout {
            Layout::Standard(_) => {
                let Some(items) = self.sd_items(p) else {
                    return 0;
                };
                let locked = d.attrs.locked;
                let remembered = if locked { d.attrs.proto(slot) } else { None };
                let cell = &mut items[slot];
                let populated = cell.is_some() || remembered.is_some();
                let same = match cell {
                    Some(c) => c.same_item_same_components(stack),
                    None => remembered.is_some_and(|i| i == &*stack.item),
                };
                if !engine::can_store(populated, same, locked, manual) {
                    return 0;
                }
                let cap = engine::slot_capacity(&d.block, &d.ups, cfg(), max);
                let count = cell.as_ref().map_or(0, |c| c.count);
                let ins = engine::insert(count, cap, stack.count, &d.ups);
                if ins.consumed == 0 {
                    return 0;
                }
                match cell {
                    Some(c) => c.count += ins.stored,
                    None => {
                        let mut s = stack.clone();
                        s.count = ins.stored.max(1);
                        *cell = Some(s);
                    }
                }
                if locked {
                    d.attrs.set_proto(slot, Some(&stack.item.to_string()));
                }
                ins.consumed
            }
            Layout::Compacting(n) => {
                // Only plain items have tiers (no components).
                if stack.damage != 0 || stack.data().is_some() {
                    return 0;
                }
                let existing = self.sd_chain(p, d);
                let (chain, pool, fresh) = match existing {
                    Some((c, pool)) => (c, pool, false),
                    None => (engine::chain_around(&stack.item, n, rules()), 0, true),
                };
                let Some(tier) = chain.tier_of(&stack.item) else {
                    return 0;
                };
                // A filled drawer takes an item on its own tier's slot; an empty one anywhere.
                if !fresh && slot != tier {
                    return 0;
                }
                let top = self.sd_max_stack(&chain.items[0]);
                let cap = engine::pool_capacity(&chain, &d.ups, &d.block, cfg(), top);
                let (used, pool) =
                    engine::pool_insert(pool, cap, chain.rates[tier], stack.count, &d.ups);
                if used == 0 {
                    return 0;
                }
                self.sd_write_pool(p, d, &chain, pool);
                used
            }
        };
        stack.count -= consumed;
        self.sd_changed(p);
        consumed
    }

    // ---- taking items out ----

    /// The count and the item of what slot `slot` shows (compacting: the tier's share of the pool).
    fn sd_slot_info(&mut self, p: [i32; 3], d: &Drawer, slot: usize) -> Option<(String, u32)> {
        match d.block.layout {
            Layout::Standard(_) => {
                let items = self.sd_items(p)?;
                let s = items.get(slot)?.as_ref()?;
                Some((s.item.to_string(), s.count))
            }
            Layout::Compacting(_) => {
                let (chain, pool) = self.sd_chain(p, d)?;
                let item = chain.items.get(slot)?.clone();
                Some((item, pool / chain.rates[slot]))
            }
        }
    }
    /// Take up to `wanted` items out of slot `slot`.
    fn sd_take(&mut self, p: [i32; 3], d: &mut Drawer, slot: usize, wanted: u32) -> Option<ItemStack> {
        if slot >= d.block.drawers() || wanted == 0 {
            return None;
        }
        match d.block.layout {
            Layout::Standard(_) => {
                let locked = d.attrs.locked;
                let items = self.sd_items(p)?;
                let cell = items.get_mut(slot)?;
                let current = cell.as_ref()?;
                let (taken, left) = engine::extract(current.count, wanted, &d.ups);
                if taken == 0 {
                    return None;
                }
                let out = ItemStack {
                    count: taken,
                    ..current.clone()
                };
                if locked {
                    d.attrs.set_proto(slot, Some(&current.item.to_string()));
                }
                if left == 0 && !engine::is_vending(&d.ups) {
                    *cell = None;
                } else if let Some(c) = cell {
                    c.count = left.max(1);
                }
                self.sd_changed(p);
                Some(out)
            }
            Layout::Compacting(_) => {
                let (chain, pool) = self.sd_chain(p, d)?;
                let item = chain.items.get(slot)?.clone();
                let (taken, left) = engine::pool_take(pool, chain.rates[slot], wanted, &d.ups);
                if taken == 0 {
                    return None;
                }
                self.sd_write_pool(p, d, &chain, left);
                Some(ItemStack::new(&item, taken))
            }
        }
    }
    /// Hand the stack to the player, or drop what does not fit in front of the drawer.
    fn sd_give(&mut self, p: [i32; 3], stack: ItemStack) {
        if let Some(rest) = self.give(stack) {
            self.pop_resource(p, rest);
        }
        self.audio.play(
            "minecraft:entity.item.pickup",
            Some([
                f64::from(p[0]) + 0.5,
                f64::from(p[1]) + 0.5,
                f64::from(p[2]) + 0.5,
            ]),
            0.2,
            1.8,
        );
    }

    // ---- the drawer's face ----

    /// Which slot of the drawer front the pick ray hits (`getFaceSlot`); `None` off the front.
    fn sd_face_slot(&self, p: [i32; 3], d: &Drawer, hit: &super::Hit) -> Option<usize> {
        let facing = self.sd_facing_at(p)?;
        if facing_normal(facing)? != hit.face {
            return None;
        }
        let fx = (hit.point[0] - f64::from(p[0])).clamp(0.0, 1.0);
        let fy = (hit.point[1] - f64::from(p[1])).clamp(0.0, 1.0);
        let fz = (hit.point[2] - f64::from(p[2])).clamp(0.0, 1.0);
        // `u` runs to the right as seen from the front (`hitWithinX`).
        let u = match facing {
            "north" => 1.0 - fx,
            "south" => fx,
            "west" => fz,
            _ => 1.0 - fz,
        };
        engine::face_slot(d.block.layout, u, fy)
    }
    fn sd_sneaking(&self) -> bool {
        self.keys.contains(&KeyCode::ShiftLeft)
    }

    // ---- placing (trigger_block "minecraft:placed_block", BlockItem.place's end) ----

    pub(super) fn storagedrawers_placed(&mut self, trigger: &str, pos: [i32; 3], stack: &ItemStack) {
        if trigger != "minecraft:placed_block" {
            return;
        }
        let Kind::Drawer(block) = self.sd_kind_at(pos) else {
            return;
        };
        // The barrel the block copies faces where the player looks (up and down too): a
        // drawer's front is the horizontal opposite of the look (`getStateForPlacement`).
        let want = placing_facing(self.yaw);
        if self.sd_facing_at(pos) != Some(want) {
            let state = self.world.block(pos[0], pos[1], pos[2]);
            if let Some(fixed) = self.with_props(state, &[("facing", want)]) {
                let at = BlockPos::new(pos[0], pos[1], pos[2]);
                let flags =
                    level_flags::UPDATE_ALL | level_flags::UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS;
                self.with_level(|b, level, ticks| b.set_block(level, ticks, at, fixed, flags));
                let _ = self.process_level_events();
            }
        }
        self.sd_ensure(pos, &block);
        // A drawer item with data (BlockEntityTag): its items and its attributes.
        let data = stack
            .data()
            .and_then(|d| d.other(horizon_client::block_entity_item::BLOCK_ENTITY_DATA))
            .cloned();
        let mut attrs = Attrs::default();
        if let Some(tag) = data {
            let size = block.chest_size();
            let items = horizon_client::save::decode_items(tag.get("Items"), size);
            if let Some(slots) = self.sd_items(pos) {
                *slots = items;
            }
            if let Some(a) = horizon_client::save::text(tag.get("StorageDrawersAttrs"))
                .and_then(|t| Attrs::decode(&t))
            {
                attrs = a;
            }
        }
        // Quantify "show default" is false in the pack's config; a key in the off hand
        // (BlockDrawers.setPlacedBy) sets the attribute on the new drawer.
        if let Some(off) = self.offhand.as_ref()
            && let Some(key) = engine::Key::parse(&off.item)
        {
            let player = self.sd_player_text();
            match key {
                engine::Key::Drawer | engine::Key::Quantify | engine::Key::Shroud
                | engine::Key::Suspend => {
                    key.apply(&mut attrs, &player);
                }
                _ => {}
            }
        }
        if attrs.locked
            && let Some(items) = self.sd_items(pos)
        {
            for (i, cell) in items[..block.drawers()].iter().enumerate() {
                if let Some(c) = cell
                    && attrs.proto(i).is_none()
                {
                    attrs.set_proto(i, Some(&c.item.to_string()));
                }
            }
        }
        self.sd_store_attrs(pos, attrs);
        self.sd_changed(pos);
    }

    // ---- right click (use_hand's chain: before the block's own use) ----

    pub(super) fn storagedrawers_use(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        match self.sd_kind_at(pos) {
            Kind::Drawer(_) => self.sd_use_drawer(pos, held),
            Kind::Controller => self.sd_use_controller(pos, held),
            _ => false,
        }
    }
    fn sd_use_drawer(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        let Some(hit) = self.pick() else {
            return false;
        };
        if hit.pos != pos {
            return false;
        }
        let Some(mut d) = self.sd_drawer(pos) else {
            return false;
        };
        let player = self.sd_player_text();
        // SecurityManager.hasAccess: a drawer owned by someone else only answers the unlock key.
        if !engine::has_access(&d.attrs, &player) {
            if held.and_then(|h| engine::Key::parse(&h.item)) == Some(engine::Key::PersonalUnlock) {
                d.attrs.owner = None;
                self.sd_store_attrs(pos, d.attrs);
            } else {
                self.sd_say("This drawer is protected by a personal key");
            }
            self.swing_arm();
            return true;
        }
        let slot = self.sd_face_slot(pos, &d, &hit);
        if let Some(held) = held {
            // Keys (ItemKey.useOn, any face; keyrings act as the key they hold).
            if let Some(key) = engine::Key::parse(&held.item) {
                if key == engine::Key::Puller {
                    self.sd_say("Detached drawers are not available on Horizon yet");
                    return true;
                }
                self.sd_key(pos, &mut d, key, &player);
                self.swing_arm();
                return true;
            }
            // Upgrades (not with Shift; use_hand already skips blocks then).
            if let Some(up) = Upgrade::parse(&held.item) {
                self.sd_add_upgrade(pos, &d, held, up);
                self.swing_arm();
                return true;
            }
        } else if self.sd_sneaking() {
            // The drawer's screen is not available: Shift + empty hand takes an upgrade out.
            self.sd_remove_upgrade(pos, &d);
            self.swing_arm();
            return true;
        }
        let Some(slot) = slot else {
            return false;
        };
        self.sd_put_click(pos, &mut d, slot, &player);
        self.sd_store_attrs(pos, d.attrs);
        self.swing_arm();
        true
    }
    /// `BlockEntityDrawers.interactPutItemsIntoSlot`: the held stack, or on a second click within
    /// 10 ticks the player's whole inventory; the contents when nothing was put.
    fn sd_put_click(&mut self, pos: [i32; 3], d: &mut Drawer, slot: usize, player: &str) {
        let now = self.world.game_time;
        let dim = self.sd_dim();
        let double = {
            let mut c = CLICK.lock().unwrap_or_else(|e| e.into_inner());
            let double = c.as_ref().is_some_and(|(cd, cp, t, cpl)| {
                *cd == dim && *cp == pos && now - *t < engine::DOUBLE_CLICK_TICKS && cpl == player
            });
            *c = Some((dim, pos, now, player.to_owned()));
            double
        };
        let mut moved = 0;
        if double {
            // The whole inventory, but only into a drawer that already holds the item.
            let populated = self.sd_slot_info(pos, d, slot).is_some();
            if populated {
                for i in 0..36 {
                    let Some(mut s) = self.inventory[i].take() else {
                        continue;
                    };
                    moved += self.sd_put(pos, d, slot, &mut s, true);
                    self.inventory[i] = (s.count > 0).then_some(s);
                }
            }
        } else if let Some(mut s) = self.held_mut().take() {
            moved += self.sd_put(pos, d, slot, &mut s, true);
            *self.held_mut() = (s.count > 0).then_some(s);
        }
        if moved == 0 {
            // No labels on the face yet: say what the slot holds.
            if let Some((item, count)) = self.sd_slot_info(pos, d, slot) {
                let cap = self.sd_capacity_text(pos, d, slot, &item);
                let name = item.trim_start_matches("minecraft:").replace('_', " ");
                self.sd_say(&format!("{name}: {count}{cap}"));
            }
        }
    }
    fn sd_capacity_text(&mut self, p: [i32; 3], d: &Drawer, slot: usize, item: &str) -> String {
        let cap = match d.block.layout {
            Layout::Standard(_) => {
                engine::slot_capacity(&d.block, &d.ups, cfg(), self.sd_max_stack(item))
            }
            Layout::Compacting(_) => match self.sd_chain(p, d) {
                Some((chain, _)) => {
                    let top = self.sd_max_stack(&chain.items[0]);
                    engine::pool_capacity(&chain, &d.ups, &d.block, cfg(), top)
                        / chain.rates.get(slot).copied().unwrap_or(1).max(1)
                }
                None => 0,
            },
        };
        if engine::is_vending(&d.ups) {
            " (unlimited)".to_owned()
        } else {
            format!(" / {cap}")
        }
    }
    /// A key used on a drawer.
    fn sd_key(&mut self, pos: [i32; 3], d: &mut Drawer, key: engine::Key, player: &str) {
        let was_locked = d.attrs.locked;
        if !key.apply(&mut d.attrs, player) {
            self.sd_say("This drawer belongs to someone else");
            return;
        }
        if key == engine::Key::Drawer && d.attrs.locked != was_locked {
            self.sd_sync_locks(pos, d);
        }
        self.sd_store_attrs(pos, d.attrs.clone());
    }
    /// A locked drawer remembers what its slots hold; an unlocked one forgets.
    fn sd_sync_locks(&mut self, pos: [i32; 3], d: &mut Drawer) {
        d.attrs.protos.clear();
        if !d.attrs.locked {
            return;
        }
        let n = d.block.drawers();
        let found: Vec<Option<String>> = match self.sd_items(pos) {
            Some(items) => items[..n]
                .iter()
                .map(|c| c.as_ref().map(|s| s.item.to_string()))
                .collect(),
            None => return,
        };
        match d.block.layout {
            Layout::Standard(_) => {
                for (i, item) in found.iter().enumerate() {
                    if let Some(item) = item {
                        d.attrs.set_proto(i, Some(item));
                    }
                }
            }
            Layout::Compacting(_) => {
                if let Some(item) = found.into_iter().flatten().next() {
                    d.attrs.set_proto(0, Some(&item));
                }
            }
        }
    }

    // ---- upgrades ----

    /// `BlockDrawers.useSlot`'s upgrade branch: one upgrade into a free upgrade slot.
    fn sd_add_upgrade(&mut self, pos: [i32; 3], d: &Drawer, held: &ItemStack, up: Upgrade) {
        if !engine::can_add_upgrade(&d.ups, up) {
            self.sd_say("That upgrade cannot be installed in this drawer");
            return;
        }
        // One stack: what is inside must fit in one stack.
        if up == Upgrade::OneStack {
            let mut with = d.ups.clone();
            with.push(up);
            if !self.sd_contents_fit(pos, d, &with) {
                self.sd_say("That upgrade cannot be installed in this drawer");
                return;
            }
        }
        let n = d.block.drawers();
        let Some(items) = self.sd_items(pos) else {
            return;
        };
        let Some(free) = (n..n + engine::UPGRADE_SLOTS).find(|&i| items[i].is_none()) else {
            self.sd_say("This drawer cannot accept any more upgrades");
            return;
        };
        items[free] = Some(ItemStack {
            count: 1,
            ..held.clone()
        });
        self.consume_held();
        self.sd_changed(pos);
    }
    /// Shift + empty hand: the last upgrade comes out (`canRemoveUpgrade`: a storage upgrade only
    /// while what is stored still fits).
    fn sd_remove_upgrade(&mut self, pos: [i32; 3], d: &Drawer) {
        let n = d.block.drawers();
        let Some(items) = self.sd_items(pos) else {
            return;
        };
        let Some(last) = (n..n + engine::UPGRADE_SLOTS).rev().find(|&i| items[i].is_some()) else {
            return;
        };
        let item = items[last].as_ref().map(|s| s.item.to_string()).unwrap_or_default();
        let mut without: Vec<Upgrade> = Vec::new();
        let mut skipped = false;
        for u in &d.ups {
            if !skipped && Some(*u) == Upgrade::parse(&item) {
                skipped = true;
                continue;
            }
            without.push(*u);
        }
        if !self.sd_contents_fit(pos, d, &without) {
            self.sd_say("This upgrade cannot be removed while the drawer holds that much");
            return;
        }
        let Some(items) = self.sd_items(pos) else {
            return;
        };
        let Some(out) = items[last].take() else {
            return;
        };
        self.sd_changed(pos);
        self.sd_give(pos, out);
    }
    /// `stackCapacityCheck`: every slot's count within the capacity the upgrades would give.
    fn sd_contents_fit(&mut self, pos: [i32; 3], d: &Drawer, ups: &[Upgrade]) -> bool {
        let n = d.block.drawers();
        match d.block.layout {
            Layout::Standard(_) => {
                let counts: Vec<(String, u32)> = match self.sd_items(pos) {
                    Some(items) => items[..n]
                        .iter()
                        .flatten()
                        .map(|s| (s.item.to_string(), s.count))
                        .collect(),
                    None => return true,
                };
                counts.iter().all(|(item, count)| {
                    *count <= engine::slot_capacity(&d.block, ups, cfg(), self.sd_max_stack(item))
                })
            }
            Layout::Compacting(_) => match self.sd_chain(pos, d) {
                Some((chain, pool)) => {
                    let top = self.sd_max_stack(&chain.items[0]);
                    pool <= engine::pool_capacity(&chain, ups, &d.block, cfg(), top)
                }
                None => true,
            },
        }
    }

    // ---- left click (attack_block: a click starts the mining) ----

    pub(super) fn storagedrawers_attack(&mut self, pos: [i32; 3]) {
        if !matches!(self.sd_kind_at(pos), Kind::Drawer(_)) {
            return;
        }
        let Some(hit) = self.pick() else {
            return;
        };
        if hit.pos != pos {
            return;
        }
        let Some(mut d) = self.sd_drawer(pos) else {
            return;
        };
        let player = self.sd_player_text();
        if !engine::has_access(&d.attrs, &player) {
            return;
        }
        let Some(slot) = self.sd_face_slot(pos, &d, &hit) else {
            return;
        };
        // FaceSlotBlock.leftAction: one item; with Shift a whole stack of it.
        let Some((item, _)) = self.sd_slot_info(pos, &d, slot) else {
            return;
        };
        let wanted = if self.sd_sneaking() {
            self.sd_max_stack(&item)
        } else {
            1
        };
        if let Some(out) = self.sd_take(pos, &mut d, slot, wanted) {
            self.sd_store_attrs(pos, d.attrs);
            self.sd_give(pos, out);
        }
    }

    // ---- the controller ----

    fn sd_use_controller(&mut self, pos: [i32; 3], held: Option<&ItemStack>) -> bool {
        let Some(hit) = self.pick() else {
            return false;
        };
        if hit.pos != pos {
            return false;
        }
        let player = self.sd_player_text();
        // Keys act on every drawer of the network (BlockController.toggle).
        if let Some(h) = held
            && let Some(key) = engine::Key::parse(&h.item)
            && key != engine::Key::Puller
        {
            self.sd_controller_key(pos, key, &player);
            self.swing_arm();
            return true;
        }
        // Only the front takes items; the other faces just absorb the click (CONSUME).
        let front = self.sd_facing_at(pos).and_then(facing_normal);
        if front != Some(hit.face) {
            return true;
        }
        let now = self.world.game_time;
        let dim = self.sd_dim();
        let double = {
            let mut c = CLICK.lock().unwrap_or_else(|e| e.into_inner());
            let double = c.as_ref().is_some_and(|(cd, cp, t, cpl)| {
                *cd == dim && *cp == pos && now - *t < engine::DOUBLE_CLICK_TICKS && *cpl == player
            });
            *c = Some((dim, pos, now, player.clone()));
            double
        };
        let network = self.sd_network(pos);
        if double {
            for i in 0..36 {
                let Some(mut s) = self.inventory[i].take() else {
                    continue;
                };
                self.sd_insert_network(&network, &mut s, &player);
                self.inventory[i] = (s.count > 0).then_some(s);
            }
        } else if let Some(mut s) = self.held_mut().take() {
            self.sd_insert_network(&network, &mut s, &player);
            *self.held_mut() = (s.count > 0).then_some(s);
        }
        self.swing_arm();
        true
    }
    /// The drawers connected to the controller (`BlockEntityController.populateRoot`).
    fn sd_network(&self, root: [i32; 3]) -> Vec<[i32; 3]> {
        engine::network(root, cfg().controller_range, |p| {
            match self.sd_kind_at(p) {
                Kind::Drawer(_) => engine::Node::Drawer,
                Kind::Controller | Kind::Networked => engine::Node::Other,
                Kind::Other => engine::Node::None,
            }
        })
    }
    fn sd_controller_key(&mut self, root: [i32; 3], key: engine::Key, player: &str) {
        let network = self.sd_network(root);
        // The group takes the state of the first drawer, flipped (`toggle*` on the group).
        let mut first: Option<Attrs> = None;
        for p in network {
            let Some(mut d) = self.sd_drawer(p) else {
                continue;
            };
            if !engine::has_access(&d.attrs, player) {
                continue;
            }
            let was = d.attrs.clone();
            if let Some(f) = &first {
                match key {
                    engine::Key::Drawer => d.attrs.locked = !f.locked,
                    engine::Key::Quantify => d.attrs.quantify = !f.quantify,
                    engine::Key::Shroud => d.attrs.concealed = !f.concealed,
                    engine::Key::Suspend => d.attrs.suspended = !f.suspended,
                    _ => {
                        key.apply(&mut d.attrs, player);
                    }
                }
            } else {
                key.apply(&mut d.attrs, player);
                first = Some(was.clone());
            }
            if key == engine::Key::Drawer && d.attrs.locked != was.locked {
                self.sd_sync_locks(p, &mut d);
            }
            self.sd_store_attrs(p, d.attrs);
        }
    }
    /// `DrawerItemRepository.insertItem` over the network: drawers that already hold the item
    /// first, by priority, then empty ones.
    fn sd_insert_network(&mut self, network: &[[i32; 3]], stack: &mut ItemStack, player: &str) {
        let mut order: Vec<([i32; 3], i32)> = network
            .iter()
            .map(|p| (*p, self.sd_attrs(*p).priority))
            .collect();
        order.sort_by_key(|(_, prio)| std::cmp::Reverse(*prio));
        for pass in 0..2 {
            for (p, _) in &order {
                if stack.count == 0 {
                    return;
                }
                let Some(mut d) = self.sd_drawer(*p) else {
                    continue;
                };
                if !engine::has_access(&d.attrs, player) || d.attrs.suspended {
                    continue;
                }
                for slot in 0..d.block.drawers() {
                    if stack.count == 0 {
                        break;
                    }
                    let holds = self
                        .sd_slot_info(*p, &d, slot)
                        .is_some_and(|(item, _)| *item == *stack.item);
                    if (pass == 0) != holds {
                        continue;
                    }
                    // A drawer slot that is empty and locked only takes its item (automation).
                    self.sd_put(*p, &mut d, slot, stack, false);
                }
                self.sd_store_attrs(*p, d.attrs);
            }
        }
    }

    // ---- hoppers and other automation (machines.rs: try_move_in_item / take_one / is_full) ----

    /// `try_move_in_item` into a drawer: the rules decide, not the vanilla stack limit. `Some`
    /// is the remainder (`None` inside: all went in); `None` when `pos` is not a drawer.
    pub(super) fn storagedrawers_move_in(
        &mut self,
        pos: (i32, i32, i32),
        slot: usize,
        stack: &ItemStack,
    ) -> Option<Option<ItemStack>> {
        let p = [pos.0, pos.1, pos.2];
        let mut d = self.sd_drawer(p)?;
        let mut rest = stack.clone();
        // Upgrade slots are not for items.
        if slot >= d.block.drawers() || d.attrs.suspended {
            return Some(Some(rest));
        }
        self.sd_put(p, &mut d, slot, &mut rest, false);
        self.sd_store_attrs(p, d.attrs);
        Some((rest.count > 0).then_some(rest))
    }
    /// Whether an upgrade slot of a drawer must not give its item to a hopper.
    pub(super) fn storagedrawers_protects(&self, pos: (i32, i32, i32), slot: usize) -> bool {
        match self.sd_kind_at([pos.0, pos.1, pos.2]) {
            Kind::Drawer(b) => slot >= b.drawers() || self.sd_attrs([pos.0, pos.1, pos.2]).suspended,
            _ => false,
        }
    }

    // ---- breaking (BlockDrawers.getMainDrop: KEEP) ----

    /// A drawer broken by a player keeps what it holds in the dropped item (`dropMode = KEEP`):
    /// the loot table's drawer gets the block entity's data through `Pending::copied`, and the
    /// block entity goes before the removal spills it.
    pub(super) fn storagedrawers_breaking(&mut self, pos: [i32; 3]) {
        let Kind::Drawer(block) = self.sd_kind_at(pos) else {
            return;
        };
        // Only when the break goes ahead (break_block_as_player's own checks); a creative
        // break drops nothing, so the contents spill as they would from a chest.
        if self.creative
            || !self.ui.rules.bool("block_drops")
            || !self.may_build()
            || !self
                .res
                .items()
                .can_destroy_block(self.held(), self.creative)
        {
            return;
        }
        let key = key_of(pos);
        let Some(BlockEntity::Chest(items)) = self.world.block_entities.get(&key) else {
            return;
        };
        let attrs = self.sd_attrs(pos);
        let empty = items.iter().all(Option::is_none) && attrs.is_default();
        if empty {
            return;
        }
        let name = self.res.facts().get(self.world.block(pos[0], pos[1], pos[2])).name.clone();
        let mut size_ok = items.clone();
        size_ok.resize(block.chest_size(), None);
        let mut tag = horizon_world::nbt::Tag::compound([
            ("id", horizon_world::nbt::Tag::String((&*name).into())),
            ("Items", horizon_client::save::encode_items(&size_ok)),
        ]);
        if let horizon_world::nbt::Tag::Compound(map) = &mut tag
            && !attrs.is_default()
        {
            map.insert(
                "StorageDrawersAttrs".into(),
                horizon_world::nbt::Tag::String(attrs.encode().as_str().into()),
            );
        }
        let mut patch = horizon_world::nbt::Compound::new();
        patch.insert(horizon_client::block_entity_item::BLOCK_ENTITY_DATA.into(), tag);
        self.pending.copied.insert(pos, (name.to_string(), patch));
        self.world.block_entities.remove(&key);
        self.sd_store_attrs(pos, Attrs::default());
        self.sd_changed(pos);
    }

    // ---- the world's file ----

    pub(super) fn storagedrawers_load(&mut self) {
        let path = self.world_root.join("data/storagedrawers/drawers.txt");
        let text = std::fs::read_to_string(path).unwrap_or_default();
        with_state(|s| *s = engine::State::from_text(&text));
    }
    pub(super) fn storagedrawers_save(&mut self) {
        if self.world_root.as_os_str().is_empty() {
            return;
        }
        let dim = self.sd_dim();
        // Forget drawers that are gone: only in loaded chunks of this dimension.
        let gone: Vec<(String, [i32; 3])> = with_state(|s| {
            s.drawers
                .keys()
                .filter(|(d, p)| {
                    *d == dim
                        && self.world.chunks.get(&(p[0] >> 4, p[2] >> 4)).is_some()
                        && !matches!(self.sd_kind_at(*p), Kind::Drawer(_))
                })
                .cloned()
                .collect()
        });
        let text = with_state(|s| {
            for g in &gone {
                s.drawers.remove(g);
            }
            s.to_text()
        });
        let dir = self.world_root.join("data/storagedrawers");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let tmp = dir.join("drawers.txt.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join("drawers.txt"));
        }
    }

    // ---- the labels (BlockEntityDrawersRenderer): the stored items on the front ----

    pub(super) fn storagedrawers_faces(
        &self,
        camera: glam::DVec3,
        out: &mut Vec<horizon_client::mesher::Vertex>,
    ) {
        for (key, entity) in &self.world.block_entities {
            let BlockEntity::Chest(items) = entity else {
                continue;
            };
            let p = [key.0, key.1, key.2];
            let Kind::Drawer(block) = self.sd_kind_at(p) else {
                continue;
            };
            if items.len() < block.chest_size() {
                continue;
            }
            let corner = glam::DVec3::new(f64::from(key.0), f64::from(key.1), f64::from(key.2));
            if (corner + glam::DVec3::splat(0.5)).distance_squared(camera) >= 24.0 * 24.0 {
                continue;
            }
            let Some(facing) = self.sd_facing_at(p) else {
                continue;
            };
            if self.sd_attrs(p).concealed {
                continue;
            }
            // Direction.get2DDataValue: south 0, west 1, north 2, east 3.
            let k = match facing {
                "west" => 1,
                "north" => 2,
                "east" => 3,
                _ => 0,
            };
            let light =
                horizon_client::block_entity_render::vertex_light(self.world.light(p[0], p[1], p[2]));
            // What each slot shows, and where on the face (fractions: right, up).
            let shown: Vec<(ItemStack, f32, f32, f32)> = match block.layout {
                Layout::Standard(n) => items[..n]
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        let (x, y, size) = match n {
                            1 => (0.5, 0.5, 0.5),
                            2 => (0.5, if i == 0 { 0.74 } else { 0.26 }, 0.36),
                            _ => (
                                if i % 2 == 0 { 0.27 } else { 0.73 },
                                if i < 2 { 0.74 } else { 0.26 },
                                0.3,
                            ),
                        };
                        Some((s.clone()?, x, y, size))
                    })
                    .collect(),
                Layout::Compacting(n) => {
                    let Some((chain, pool)) = items[..n].iter().flatten().next().map(|s| {
                        (engine::chain_from_smallest(&s.item, n, rules()), s.count)
                    }) else {
                        continue;
                    };
                    let _ = pool;
                    chain
                        .items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let (x, y, size) = match (n, i) {
                                (_, 0) => (0.5, 0.74, 0.36),
                                (2, _) => (0.5, 0.26, 0.36),
                                (_, 1) => (0.27, 0.26, 0.3),
                                _ => (0.73, 0.26, 0.3),
                            };
                            (ItemStack::new(item, 1), x, y, size)
                        })
                        .collect()
                }
            };
            for (stack, x, y, size) in shown {
                let item = self.resolve_item(
                    &stack,
                    horizon_assets::item_model::DisplayContext::Fixed,
                    None,
                    0,
                );
                if item.is_empty() {
                    continue;
                }
                // The front is +Z of the block turned by its facing; items face -Z in their own
                // frame (the campfire turns them by 90 degrees about X to lie up), so turn 180.
                let pose = glam::Mat4::from_translation(
                    (corner - camera).as_vec3() + glam::Vec3::splat(0.5),
                ) * glam::Mat4::from_rotation_y((-(k as f32) * 90.0).to_radians())
                    * glam::Mat4::from_translation(glam::Vec3::new(x - 0.5, y - 0.5, 0.5 + 0.004))
                    * glam::Mat4::from_rotation_y(180f32.to_radians())
                    * glam::Mat4::from_scale(glam::Vec3::new(size, size, 0.01));
                self.submit_item_world(&item, pose, light, out);
            }
        }
    }
}

/// The block entity size of a block of the mod, for the hooks.
pub(crate) fn storagedrawers_size(name: &str) -> Option<usize> {
    engine::chest_size(name, cfg())
}
/// The upgrade slots start here in a drawer's block entity (`None`: not a drawer).
pub(crate) fn storagedrawers_upgrades_from(name: &str) -> Option<usize> {
    match engine::kind(name, cfg()) {
        Kind::Drawer(b) => Some(b.drawers()),
        _ => None,
    }
}

/// `HopperBlockEntity.isFullContainer` for a drawer's slots: `Some(full)` for a drawer's block
/// entity, `None` for anything else. `max_stack` is the item's stack size.
pub(crate) fn storagedrawers_full(
    name: &str,
    items: &[Option<ItemStack>],
    max_stack: impl Fn(&str) -> u32,
) -> Option<bool> {
    let Kind::Drawer(block) = engine::kind(name, cfg()) else {
        return None;
    };
    let n = block.drawers();
    if items.len() < block.chest_size() {
        return None;
    }
    let ids: Vec<Option<&str>> = items[n..].iter().map(|s| s.as_ref().map(|s| &*s.item)).collect();
    let ups = engine::upgrades_of(&ids);
    Some(match block.layout {
        Layout::Standard(_) => items[..n].iter().all(|c| {
            c.as_ref().is_some_and(|s| {
                s.count >= engine::slot_capacity(&block, &ups, cfg(), max_stack(&s.item))
            })
        }),
        Layout::Compacting(_) => match items[..n].iter().flatten().next() {
            Some(s) => {
                let chain = engine::chain_from_smallest(&s.item, n, rules());
                s.count >= engine::pool_capacity(&chain, &ups, &block, cfg(), max_stack(&chain.items[0]))
            }
            None => false,
        },
    })
}
