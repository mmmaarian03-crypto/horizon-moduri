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
