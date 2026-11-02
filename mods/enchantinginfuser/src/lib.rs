// Enchanting Infuser for Horizon: the rules, without any game code.
// From the public source (Fuzss/enchantinginfuser, branch 1.20.1, licence MPL-2.0; the jar the pack plays is
// EnchantingInfuser-v8.0.3-1.20.1-Forge.jar): `InfuserMenu`, `EnchantmentUtil`, `InfuserBlock`,
// `ChiseledBookshelfHelper`, `EnchantStatsProviderImpl`, `ServerConfig`, `InfuserScreen` (layout).
// the pack's values: `the 1.20.1 pack/config/enchantinginfuser-server.toml` (`config` below).
// Apotheosis is in the pack but not ported to Horizon yet: the infuser uses its own vanilla
// provider, as the original does without Apotheosis.
// This file is pasted into the game as `crate::game::enchantinginfuser::engine` (tools/hnmod.py bundle) and is
// also the `horizon_enchantinginfuser` crate the tests use: plain Rust, no game types, no inner attributes.

pub type Pos = [i32; 3];

pub const NORMAL: &str = "enchantinginfuser:enchanting_infuser";
pub const ADVANCED: &str = "enchantinginfuser:advanced_enchanting_infuser";
/// The infuser's one slot in the game's slot numbering (container slots start at 300).
pub const CONTAINER_SLOT: usize = 300;

/// `ServerConfig.ModifiableItems`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifiable {
    Unenchanted,
    All,
    FullDurability,
}

/// `ServerConfig.AllowedRepairItems`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repairs {
    Everything,
    ToolsAndArmor,
    Nothing,
}

/// `ServerConfig.InfuserConfig` (with its `types`, `repair`, `costs`, `power` sections).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InfuserConfig {
    pub maximum_bookshelves: i32,
    pub modifying: Modifiable,
    pub allow_books: bool,
    pub repairing: Repairs,
    pub increase_anvil_repair_cost: bool,
    pub allow_curses: bool,
    pub allow_treasure: bool,
    pub allow_untradeable: bool,
    pub allow_anvil: bool,
    pub allow_undiscoverable: bool,
    pub repair_step_multiplier: f64,
    pub repair_percentage_step: f64,
    pub common_cost: i32,
    pub uncommon_cost: i32,
    pub rare_cost: i32,
    pub very_rare_cost: i32,
    pub double_uniques: bool,
    pub maximum_cost: i32,
    pub scale_costs_by_vanilla_only: bool,
    pub common_power: f64,
    pub uncommon_power: f64,
    pub rare_power: f64,
    pub very_rare_power: f64,
    pub rarity_range: f64,
    pub treasure_power: f64,
    pub curse_power: f64,
    pub untradeable_power: f64,
    pub undiscoverable_power: f64,
}

/// the pack's `enchantinginfuser-server.toml`.
pub mod config {
    use super::{InfuserConfig, Modifiable, Repairs};
    /// `[normal_infuser]` (lines 102-200).
    pub const NORMAL: InfuserConfig = InfuserConfig {
        maximum_bookshelves: 15,         // :113
        modifying: Modifiable::Unenchanted, // :107
        allow_books: false,              // :117
        repairing: Repairs::Nothing,     // :104
        increase_anvil_repair_cost: false, // :115
        allow_curses: false,             // :122
        allow_treasure: false,           // :124
        allow_untradeable: false,        // :127
        allow_anvil: false,              // :129
        allow_undiscoverable: false,     // :132
        repair_step_multiplier: 2.0,     // :137
        repair_percentage_step: 0.25,    // :140
        very_rare_cost: 5,               // :147
        double_uniques: true,            // :149
        rare_cost: 4,                    // :152
        maximum_cost: 30,                // :155
        scale_costs_by_vanilla_only: true, // :157
        uncommon_cost: 3,                // :160
        common_cost: 2,                  // :163
        rare_power: 0.4,                 // :170
        untradeable_power: 0.9,          // :174
        rarity_range: 0.4,               // :177
        undiscoverable_power: 0.9,       // :181
        treasure_power: 0.95,            // :185
        curse_power: 1.0,                // :189
        common_power: -0.2,              // :192
        very_rare_power: 0.6,            // :195
        uncommon_power: 0.2,             // :198
    };
    /// `[advanced_infuser]` (lines 1-100).
    pub const ADVANCED: InfuserConfig = InfuserConfig {
        maximum_bookshelves: 15,         // :14
        modifying: Modifiable::All,      // :8
        allow_books: true,               // :18
        repairing: Repairs::ToolsAndArmor, // :5
        increase_anvil_repair_cost: false, // :16
        allow_curses: false,             // :23
        allow_treasure: false,           // :25
        allow_untradeable: false,        // :28
        allow_anvil: true,               // :30
        allow_undiscoverable: false,     // :33
        repair_step_multiplier: 2.0,     // :38
        repair_percentage_step: 0.25,    // :41
        very_rare_cost: 5,               // :48
        double_uniques: true,            // :50
        rare_cost: 4,                    // :53
        maximum_cost: 20,                // :56
        scale_costs_by_vanilla_only: true, // :58
        uncommon_cost: 3,                // :61
        common_cost: 2,                  // :64
        rare_power: 0.4,                 // :71
        untradeable_power: 0.9,          // :75
        rarity_range: 0.4,               // :78
        undiscoverable_power: 0.9,       // :82
        treasure_power: 0.95,            // :86
        curse_power: 1.0,                // :90
        common_power: -0.2,              // :93
        very_rare_power: 0.6,            // :96
        uncommon_power: 0.2,             // :99
    };
}

/// The config of an infuser block.
pub fn config_of(block: &str) -> Option<&'static InfuserConfig> {
    match block {
        NORMAL => Some(&config::NORMAL),
        ADVANCED => Some(&config::ADVANCED),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------- enchantments

/// `Enchantment.Rarity`, from the 26.3 weight (`COMMON` 10, `UNCOMMON` 5, `RARE` 2, `VERY_RARE` 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    VeryRare,
}

impl Rarity {
    pub fn from_weight(weight: u32) -> Self {
        match weight {
            w if w >= 10 => Rarity::Common,
            w if w >= 5 => Rarity::Uncommon,
            w if w >= 2 => Rarity::Rare,
            _ => Rarity::VeryRare,
        }
    }
}

/// An enchantment as the infuser reads it (`EnchantStatsProviderImpl`, 26.3 data and tags).
#[derive(Clone, Debug, PartialEq)]
pub struct Ench {
    pub id: String,
    pub rarity: Rarity,
    pub max_level: u32,
    /// `getMinCost(level)` = base + per_level * (level - 1).
    pub min_cost_base: i32,
    pub min_cost_per_level: i32,
    /// `#minecraft:curse`
    pub curse: bool,
    /// `#minecraft:treasure` (`isTreasureOnly`)
    pub treasure: bool,
    /// `#minecraft:tradeable` (`isTradeable`: not soul speed, swift sneak)
    pub tradeable: bool,
    /// `#minecraft:on_random_loot` (`isDiscoverable`: not soul speed, swift sneak)
    pub discoverable: bool,
}

impl Ench {
    pub fn min_level(&self) -> u32 {
        1
    }
    pub fn min_cost(&self, level: u32) -> i32 {
        self.min_cost_base + self.min_cost_per_level * (level as i32 - 1)
    }
    pub fn vanilla(&self) -> bool {
        self.id.starts_with("minecraft:")
    }
}

/// What the infuser asks about enchantments and items (the adapter answers from the game's data).
pub trait Catalog {
    /// Every enchantment, in registry order (`BuiltInRegistries.ENCHANTMENT`).
    fn enchantments(&self) -> Vec<Ench>;
    /// `Enchantment.isCompatibleWith` (exclusive sets).
    fn compatible(&self, a: &str, b: &str) -> bool;
    /// `canApplyAtEnchantingTable` (the primary items) or, with `anvil`, `canEnchant` (supported items).
    fn applies(&self, ench: &str, item: &str, anvil: bool) -> bool;
}

/// The infuser's item, as the rules read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Item {
    pub id: String,
    pub count: u32,
    pub max_stack: u32,
    pub max_damage: u32,
    pub damage: u32,
    /// Its enchantments (stored ones for an enchanted book).
    pub enchantments: Vec<(String, u32)>,
    /// `TieredItem` (pickaxes, axes, shovels, hoes, swords) or `ArmorItem`.
    pub tool_or_armor: bool,
}

impl Item {
    pub fn book(&self) -> bool {
        self.id == "minecraft:book"
    }
    pub fn enchanted_book(&self) -> bool {
        self.id == "minecraft:enchanted_book"
    }
    /// `isDamaged`
    pub fn damaged(&self) -> bool {
        self.max_damage > 0 && self.damage > 0
    }
    /// `isEnchanted`
    pub fn enchanted(&self) -> bool {
        !self.enchanted_book() && !self.enchantments.is_empty()
    }
    /// `Item.isEnchantable(stack)`: stack size 1 and damageable (a book: a single one).
    fn item_enchantable(&self) -> bool {
        if self.book() {
            return self.count == 1;
        }
        self.max_stack == 1 && self.max_damage > 0
    }
}

/// `ModifiableItems.predicate`.
pub fn modifiable(m: Modifiable, item: &Item) -> bool {
    let unenchanted = item.item_enchantable() && !item.enchanted();
    let all = unenchanted || (item.item_enchantable() && item.enchanted());
    match m {
        Modifiable::Unenchanted => unenchanted,
        Modifiable::All => all,
        Modifiable::FullDurability => !item.damaged() && all,
    }
}

/// `InfuserMenu.mayEnchantStack`.
pub fn may_enchant(cfg: &InfuserConfig, item: &Item) -> bool {
    if cfg.allow_books {
        if item.book() {
            return true;
        } else if item.enchanted_book() {
            return cfg.modifying != Modifiable::Unenchanted;
        }
    } else if item.book() || item.enchanted_book() {
        return false;
    }
    modifiable(cfg.modifying, item)
}

/// `EnchantmentUtil.getAvailableEnchantments`.
pub fn available(cfg: &InfuserConfig, catalog: &impl Catalog, item: &Item) -> Vec<Ench> {
    let book = item.book() || item.enchanted_book();
    catalog
        .enchantments()
        .into_iter()
        .filter(|e| catalog.applies(&e.id, &item.id, cfg.allow_anvil) || book)
        .filter(|e| {
            if !e.discoverable {
                cfg.allow_undiscoverable
            } else if !e.tradeable {
                cfg.allow_untradeable
            } else if e.curse {
                cfg.allow_curses
            } else if e.treasure {
                cfg.allow_treasure
            } else {
                true
            }
        })
        .collect()
}

/// `AllowedRepairItems.isAllowedToRepair`.
pub fn repair_allowed(cfg: &InfuserConfig, item: Option<&Item>) -> bool {
    let Some(item) = item.filter(|i| i.damaged()) else {
        return false;
    };
    match cfg.repairing {
        Repairs::ToolsAndArmor => item.tool_or_armor,
        Repairs::Everything => true,
        Repairs::Nothing => false,
    }
}

/// `InfuserMenu.calculateRepairCost`: `ceil(ceil(damage / (maxDamage * step)) * multiplier)`.
pub fn repair_cost(cfg: &InfuserConfig, item: Option<&Item>) -> i32 {
    if !repair_allowed(cfg, item) {
        return 0;
    }
    let item = item.expect("allowed");
    let step = f64::from(item.max_damage) * cfg.repair_percentage_step;
    ((f64::from(item.damage) / step).ceil() * cfg.repair_step_multiplier).ceil() as i32
}

/// `Math.round(double)`: half up.
fn java_round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

// ------------------------------------------------------------------------------------- power

/// `EnchantmentTableBlock.BOOKSHELF_OFFSETS`: the ring two blocks away, on the infuser's layer and above.
pub fn bookshelf_offsets() -> Vec<Pos> {
    let mut out = Vec::new();
    for x in -2..=2 {
        for y in 0..=1 {
            for z in -2..=2 {
                if i32::abs(x) == 2 || i32::abs(z) == 2 {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

/// What stands at one offset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spot {
    /// `getEnchantPowerBonus` (a bookshelf: 1).
    pub bonus: f32,
    /// A chiseled bookshelf facing the infuser: its books (`ChiseledBookshelfHelper.findValidBooks`).
    pub chiseled_books: i32,
    /// The block halfway between is a full collision cube (it blocks the shelf).
    pub between_full: bool,
}

/// `ChiseledBookshelfHelper`: a chiseled bookshelf at `offset` counts when its front (`facing`) looks
/// back at the infuser.
pub fn chiseled_faces_infuser(offset: Pos, facing: &str) -> bool {
    // direction = facing.getOpposite(); axisOffset along its axis; |2| with the direction's sign.
    let (axis, step) = match facing {
        "north" => (2, 1),
        "south" => (2, -1),
        "west" => (0, 1),
        "east" => (0, -1),
        _ => return false,
    };
    let along = offset[axis];
    along.abs() == 2 && along.signum() == step
}

/// `InfuserMenu.getAvailablePower`.
pub fn power(cfg: &InfuserConfig, spot: impl Fn(Pos) -> Spot) -> i32 {
    let mut enchanting_power = 0.0f32;
    let mut chiseled = 0;
    let max_power_scale = 1.0f32;
    for offset in bookshelf_offsets() {
        let s = spot(offset);
        // InfuserBlock.isValidBookShelf
        if (s.bonus == 0.0 && s.chiseled_books == 0) || s.between_full {
            continue;
        }
        if s.chiseled_books > 0 {
            chiseled += s.chiseled_books;
        } else {
            enchanting_power += s.bonus;
        }
    }
    let p = (enchanting_power + (chiseled / 3) as f32).max(0.0);
    p.min(cfg.maximum_bookshelves as f32 * max_power_scale) as i32
}

// ------------------------------------------------------------------------------------- the menu

/// `InfuserMenu`'s state for one player and one item (`setAndSyncEnchantments` + the data slots).
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub cfg: InfuserConfig,
    pub item: Option<Item>,
    /// The enchantments on offer, in registry order.
    pub enchants: Vec<Ench>,
    /// `compat[i][j]`: enchantments i and j can be on one item.
    pub compat: Vec<Vec<bool>>,
    pub levels: Vec<u32>,
    pub original: Vec<u32>,
    pub base_cost: i32,
    pub power: i32,
    pub enchant_cost: i32,
    pub repair_cost: i32,
    pub changed: bool,
}

impl Session {
    /// `slotsChanged`: the item went in or out (or the menu opened).
    pub fn new(cfg: &InfuserConfig, catalog: &impl Catalog, item: Option<Item>, power: i32) -> Self {
        let mut s = Session {
            cfg: *cfg,
            item: item.clone(),
            enchants: Vec::new(),
            compat: Vec::new(),
            levels: Vec::new(),
            original: Vec::new(),
            base_cost: 0,
            power,
            enchant_cost: 0,
            repair_cost: 0,
            changed: false,
        };
        if let Some(item) = item.as_ref().filter(|i| may_enchant(cfg, i)) {
            s.enchants = available(cfg, catalog, item);
            s.compat = s
                .enchants
                .iter()
                .map(|a| s.enchants.iter().map(|b| a.id == b.id || catalog.compatible(&a.id, &b.id)).collect())
                .collect();
            // copyEnchantmentsToMap: the item's levels of the enchantments on offer.
            s.levels = s
                .enchants
                .iter()
                .map(|e| item.enchantments.iter().find(|(id, _)| *id == e.id).map_or(0, |(_, l)| *l))
                .collect();
            s.repair_cost = repair_cost(cfg, Some(item));
        }
        s.original = s.levels.clone();
        s.base_cost = s.scaled_costs(&s.levels.clone());
        s.changed = false;
        s
    }

    pub fn max_power(&self) -> i32 {
        self.cfg.maximum_bookshelves
    }
    pub fn current_power(&self) -> i32 {
        self.power.min(self.max_power())
    }

    fn rarity_power(&self, r: Rarity) -> f64 {
        match r {
            Rarity::Common => self.cfg.common_power,
            Rarity::Uncommon => self.cfg.uncommon_power,
            Rarity::Rare => self.cfg.rare_power,
            Rarity::VeryRare => self.cfg.very_rare_power,
        }
    }
    fn rarity_cost(&self, r: Rarity) -> i32 {
        match r {
            Rarity::Common => self.cfg.common_cost,
            Rarity::Uncommon => self.cfg.uncommon_cost,
            Rarity::Rare => self.cfg.rare_cost,
            Rarity::VeryRare => self.cfg.very_rare_cost,
        }
    }

    /// `getMaxLevel`: (the power needed for the next level, the highest level allowed now).
    pub fn max_level(&self, i: usize) -> (Option<i32>, u32) {
        let e = &self.enchants[i];
        let current = self.current_power();
        let max_power = self.max_power();
        // getSpecialMaxLevel: at most one multiplier, in this order.
        let special = if e.curse {
            Some(self.cfg.curse_power)
        } else if !e.discoverable {
            Some(self.cfg.undiscoverable_power)
        } else if !e.tradeable {
            Some(self.cfg.untradeable_power)
        } else if e.treasure {
            Some(self.cfg.treasure_power)
        } else {
            None
        };
        if let Some(m) = special {
            let next = java_round(f64::from(max_power) * m) as i32;
            return if current < next { (Some(next), 0) } else { (None, e.max_level) };
        }
        let min_power = java_round(f64::from(max_power) * self.rarity_power(e.rarity)) as i32;
        if current < min_power {
            return (Some(min_power), 0);
        }
        let total = e.max_level - e.min_level();
        let range = f64::from(max_power) * self.cfg.rarity_range;
        let percentile = if total > 0 { range / f64::from(total) } else { 0.0 };
        for i in 0..=total {
            let next = max_power.min((f64::from(min_power) + f64::from(i) * percentile).ceil() as i32);
            if current < next {
                return (Some(next), e.min_level() + i - 1);
            }
        }
        (Some(max_power), e.max_level)
    }

    /// `clickEnchantmentLevelButton`: the new level, or None (refused).
    pub fn click_level(&mut self, i: usize, increase: bool) -> Option<u32> {
        let incompatible = (0..self.enchants.len()).any(|j| j != i && self.levels[j] > 0 && !self.compat[j][i]);
        if incompatible {
            return None;
        }
        let level = self.levels[i] as i64 + if increase { 1 } else { -1 };
        if level < 0 || level > i64::from(self.enchants[i].max_level) {
            return None;
        }
        if level > i64::from(self.max_level(i).1) {
            return None;
        }
        self.levels[i] = level as u32;
        self.enchant_cost = self.calculate_enchant_cost();
        Some(level as u32)
    }

    /// The level button with Shift: `do { .. } while (button.active && Screen.hasShiftDown())`.
    pub fn click_level_repeat(&mut self, i: usize, increase: bool, shift: bool) -> bool {
        let mut any = false;
        loop {
            if self.click_level(i, increase).is_none() {
                return any;
            }
            any = true;
            let b = self.buttons(i);
            let active = if increase { b.incr_active && b.incr_visible } else { b.decr_active && b.decr_visible };
            if !(shift && active) {
                return any;
            }
        }
    }

    fn calculate_enchant_cost(&mut self) -> i32 {
        self.changed = self.levels != self.original;
        let mut cost = self.scaled_costs(&self.levels) - self.base_cost;
        if cost == 0 && self.changed {
            cost += 1;
        }
        cost
    }

    fn adjusted_rarity_cost(&self, e: &Ench) -> i32 {
        let mut cost = self.rarity_cost(e.rarity);
        if self.cfg.double_uniques && (e.treasure || !e.discoverable || !e.tradeable) && !e.curse {
            cost *= 2;
        }
        cost
    }

    /// `getAllCosts`.
    fn all_costs(&self, levels: &[u32]) -> i32 {
        self.enchants
            .iter()
            .zip(levels)
            .filter(|(_, l)| **l > 0)
            .map(|(e, l)| self.adjusted_rarity_cost(e) * *l as i32)
            .sum()
    }

    /// `getTotalCosts`: every enchantment on offer, an incompatible one merged into the first it clashes
    /// with (the dearer of the two counts). The original walks a `HashMap` (no fixed order); here the
    /// registry order.
    pub fn total_costs(&self) -> i32 {
        let mut kept: Vec<(usize, Rarity, u32)> = Vec::new();
        for (i, e) in self.enchants.iter().enumerate() {
            let scale = !self.cfg.scale_costs_by_vanilla_only || e.vanilla();
            if !scale {
                continue;
            }
            let pair = (e.rarity, e.max_level);
            match kept.iter_mut().find(|(k, _, _)| !self.compat[*k][i]) {
                Some(slot) => {
                    let c1 = self.rarity_cost(slot.1) * slot.2 as i32;
                    let c2 = self.rarity_cost(pair.0) * pair.1 as i32;
                    if c2 > c1 {
                        slot.1 = pair.0;
                        slot.2 = pair.1;
                    }
                }
                None => kept.push((i, pair.0, pair.1)),
            }
        }
        kept.iter().map(|(_, r, l)| self.rarity_cost(*r) * *l as i32).sum()
    }

    /// `getScaledCosts`.
    pub fn scaled_costs(&self, levels: &[u32]) -> i32 {
        let total = f64::from(self.total_costs());
        let max_cost = self.cfg.maximum_cost;
        let book = self.item.as_ref().is_some_and(|i| i.book() || i.enchanted_book());
        if total > f64::from(max_cost) && !book {
            let ratio = f64::from(max_cost) / total;
            let min_costs: i32 = levels.iter().map(|l| *l as i32).sum();
            return (java_round(f64::from(self.all_costs(levels)) * ratio) as i32).max(min_costs);
        }
        self.all_costs(levels)
    }

    /// `canEnchant`.
    pub fn can_enchant(&self, player_levels: i32, creative: bool) -> bool {
        self.item.is_some() && self.changed && (player_levels >= self.enchant_cost || creative)
    }
    /// `canRepair`.
    pub fn can_repair(&self, player_levels: i32, creative: bool) -> bool {
        repair_allowed(&self.cfg, self.item.as_ref()) && (player_levels >= self.repair_cost || creative)
    }

    /// `calculateExperienceDelta`: the experience given back for lowered levels (`next_int(n)` is the
    /// level's random).
    pub fn experience_delta(&self, mut next_int: impl FnMut(i32) -> i32) -> i32 {
        let mut xp = 0;
        for (i, e) in self.enchants.iter().enumerate() {
            let (o, c) = (self.original[i], self.levels[i]);
            if o > c {
                xp += e.min_cost(o).max(0) - e.min_cost(c).max(0);
            }
        }
        if xp > 0 {
            let xp = (f64::from(xp) / 2.0).ceil() as i32;
            xp + next_int(xp)
        } else {
            0
        }
    }

    /// `EnchantmentUtil.setNewEnchantments`: the enchantments of the result (the item's own that are not
    /// on offer, a curse say, stay), and the result's item id (a book becomes an enchanted book and back).
    pub fn result(&self) -> Option<(String, Vec<(String, u32)>)> {
        let item = self.item.as_ref()?;
        let mut levels: Vec<(String, u32)> = self.enchants.iter().zip(&self.levels).map(|(e, l)| (e.id.clone(), *l)).collect();
        for (id, l) in &item.enchantments {
            if !levels.iter().any(|(e, _)| e == id) {
                levels.push((id.clone(), *l));
            }
        }
        levels.retain(|(_, l)| *l > 0);
        let enchanted = !levels.is_empty();
        let id = if item.enchanted_book() && !enchanted {
            "minecraft:book".to_owned()
        } else if item.book() && enchanted {
            "minecraft:enchanted_book".to_owned()
        } else {
            item.id.clone()
        };
        Some((id, levels))
    }

    /// `getSortedEntries`: by rarity (common first), then by name.
    pub fn sorted(&self, name: impl Fn(&str) -> String) -> Vec<usize> {
        let mut order: Vec<usize> = (0..self.enchants.len()).collect();
        order.sort_by(|&a, &b| {
            let (ea, eb) = (&self.enchants[a], &self.enchants[b]);
            ea.rarity.cmp(&eb.rarity).then_with(|| name(&ea.id).cmp(&name(&eb.id)))
        });
        order
    }

    /// `EnchantmentListEntry.updateButtons` + `markIncompatible`.
    pub fn buttons(&self, i: usize) -> Buttons {
        let level = self.levels[i];
        let (_, allowed) = self.max_level(i);
        let incompatible = self.incompatible_with(i);
        let compatible = incompatible.is_empty();
        let shown_level = if compatible { level } else { 0 };
        Buttons {
            decr_visible: shown_level > 0,
            incr_visible: shown_level < self.enchants[i].max_level,
            decr_active: compatible && (i64::from(shown_level) - 1) < i64::from(allowed),
            incr_active: compatible && shown_level < allowed,
        }
    }

    /// `ScrollingList.markOthersIncompatible`: the active enchantments an inactive one clashes with.
    pub fn incompatible_with(&self, i: usize) -> Vec<usize> {
        if self.levels[i] > 0 {
            return Vec::new();
        }
        (0..self.enchants.len()).filter(|&j| j != i && self.levels[j] > 0 && !self.compat[j][i]).collect()
    }
}

/// The level buttons of a list entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub decr_visible: bool,
    pub incr_visible: bool,
    pub decr_active: bool,
    pub incr_active: bool,
}

// ------------------------------------------------------------------------------------- the screen

/// `InfuserScreen` sizes and places (GUI pixels from the image's corner).
pub mod screen {
    pub const WIDTH: f32 = 220.0;
    pub const HEIGHT: f32 = 185.0;
    pub const TEXTURE_SIZE: f32 = 256.0;
    /// The list: `new ScrollingList(leftPos + 29, topPos + 17, 162, 18, 4)`.
    pub const LIST_X: f32 = 29.0;
    pub const LIST_Y: f32 = 17.0;
    pub const ITEM_W: f32 = 162.0;
    pub const ITEM_H: f32 = 18.0;
    pub const ROWS: usize = 4;
    /// The scrollbar: 197..211 x 17..89.
    pub const BAR_X: f32 = 197.0;
    pub const BAR_Y: f32 = 17.0;
    pub const BAR_W: f32 = 14.0;
    pub const BAR_H: f32 = 72.0;
    pub const BUTTON_X: f32 = 7.0;
    pub const ENCHANT_Y: f32 = 44.0;
    pub const ENCHANT_ONLY_Y: f32 = 55.0;
    pub const REPAIR_Y: f32 = 66.0;
    pub const POWER_X: f32 = 196.0;
    pub const POWER_Y: f32 = 161.0;
    pub const INVENTORY_LABEL: (f32, f32) = (30.0, 91.0);
    pub const TITLE_LABEL: (f32, f32) = (8.0, 6.0);
}

/// The game slot's 16x16 item corner in the infuser's screen (`InfuserMenu` slots).
pub fn slot_position(cfg: &InfuserConfig, slot: usize) -> Option<(f32, f32)> {
    match slot {
        CONTAINER_SLOT => Some((8.0, if cfg.repairing != Repairs::Nothing { 23.0 } else { 34.0 })),
        9..36 => {
            let i = slot - 9;
            Some((30.0 + (i % 9) as f32 * 18.0, 103.0 + (i / 9) as f32 * 18.0))
        }
        0..9 => Some((30.0 + slot as f32 * 18.0, 161.0)),
        _ => None,
    }
}

/// What a press at (mx, my) (GUI pixels from the image's corner) hits, with the list scrolled by
/// `scroll` rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Widget {
    /// A list row's minus / plus button (`entry` = the row's index in the sorted list).
    Level { entry: usize, increase: bool },
    Enchant,
    Repair,
    Scrollbar,
}

/// The widget under a press; `visible(entry)` = the row's (decr, incr) buttons shown.
pub fn widget_at(cfg: &InfuserConfig, mx: f32, my: f32, scroll: usize, rows: usize, buttons: impl Fn(usize) -> Buttons) -> Option<Widget> {
    use screen::*;
    let inside = |x: f32, y: f32, w: f32, h: f32| mx >= x && my >= y && mx < x + w && my < y + h;
    let enchant_y = if cfg.repairing != Repairs::Nothing { ENCHANT_Y } else { ENCHANT_ONLY_Y };
    if inside(BUTTON_X, enchant_y, 18.0, 18.0) {
        return Some(Widget::Enchant);
    }
    if cfg.repairing != Repairs::Nothing && inside(BUTTON_X, REPAIR_Y, 18.0, 18.0) {
        return Some(Widget::Repair);
    }
    if inside(BAR_X, BAR_Y, BAR_W, BAR_H) {
        return Some(Widget::Scrollbar);
    }
    if inside(LIST_X, LIST_Y, ITEM_W, ITEM_H * ROWS as f32) {
        let row = ((my - LIST_Y) / ITEM_H) as usize;
        let entry = scroll + row;
        if entry >= rows {
            return None;
        }
        let b = buttons(entry);
        let y = LIST_Y + ITEM_H * row as f32;
        if b.decr_visible && inside(LIST_X, y, 18.0, 18.0) {
            return Some(Widget::Level { entry, increase: false });
        }
        if b.incr_visible && inside(LIST_X + ITEM_W - 18.0, y, 18.0, 18.0) {
            return Some(Widget::Level { entry, increase: true });
        }
    }
    None
}

/// `ScrollingList.scrollTo`: the first row shown for a scrollbar position 0..1.
pub fn scroll_rows(pos: f32, rows: usize) -> usize {
    if rows > screen::ROWS {
        ((rows - screen::ROWS) as f32 * pos).round() as usize
    } else {
        0
    }
}

/// `mouseScrolled`: the new scrollbar position for a wheel step.
pub fn wheel(pos: f32, delta: f32, rows: usize) -> f32 {
    if rows <= screen::ROWS {
        return 0.0;
    }
    (pos - delta / (rows - screen::ROWS) as f32).clamp(0.0, 1.0)
}

/// `mouseDragged` / a press on the bar: the position for the cursor's height (GUI pixels from the image).
pub fn bar_position(my: f32) -> f32 {
    ((my - screen::BAR_Y - 7.5) / (screen::BAR_H - 15.0)).clamp(0.0, 1.0)
}

// ------------------------------------------------------------------------------------- the view

/// What the screen shows (the server's session, as a foundry menu section for a guest's window).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub rows: Vec<Row>,
    pub power: i32,
    pub max_power: i32,
    pub enchant_cost: i32,
    pub repair_cost: i32,
    /// An item is in the slot.
    pub has_item: bool,
    /// The levels differ from the item's (`enchantmentsChanged`).
    pub changed: bool,
    /// `AllowedRepairItems.isAllowedToRepair` for the item.
    pub repair_allowed: bool,
}

/// One row of the list, in display order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub id: String,
    pub level: u32,
    pub data_max: u32,
    pub allowed: u32,
    pub required: i32,
    pub incompatible: Vec<String>,
    pub buttons: (bool, bool, bool, bool),
}

impl Row {
    pub fn buttons(&self) -> Buttons {
        Buttons { decr_visible: self.buttons.0, incr_visible: self.buttons.1, decr_active: self.buttons.2, incr_active: self.buttons.3 }
    }
    /// `isObfuscated`: no level allowed with this power.
    pub fn obfuscated(&self) -> bool {
        self.allowed == 0
    }
}

impl View {
    /// `canEnchant` for a player with these levels.
    pub fn can_enchant(&self, player_levels: i32, creative: bool) -> bool {
        self.has_item && self.changed && (player_levels >= self.enchant_cost || creative)
    }
    /// `canRepair`.
    pub fn can_repair(&self, player_levels: i32, creative: bool) -> bool {
        self.repair_allowed && (player_levels >= self.repair_cost || creative)
    }
    /// The view of a session (`order` = `sorted`).
    pub fn of(s: &Session, order: &[usize]) -> Self {
        let rows = order
            .iter()
            .map(|&i| {
                let (required, allowed) = s.max_level(i);
                let b = s.buttons(i);
                let incompatible = s.incompatible_with(i).into_iter().map(|j| s.enchants[j].id.clone()).collect();
                Row {
                    id: s.enchants[i].id.clone(),
                    level: if s.incompatible_with(i).is_empty() { s.levels[i] } else { 0 },
                    data_max: s.enchants[i].max_level,
                    allowed,
                    required: required.unwrap_or(-1),
                    incompatible,
                    buttons: (b.decr_visible, b.incr_visible, b.decr_active, b.incr_active),
                }
            })
            .collect();
        View {
            rows,
            power: s.current_power(),
            max_power: s.max_power(),
            enchant_cost: s.enchant_cost,
            repair_cost: s.repair_cost,
            has_item: s.item.is_some(),
            changed: s.changed,
            repair_allowed: repair_allowed(&s.cfg, s.item.as_ref()),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.int(self.power);
        w.int(self.max_power);
        w.int(self.enchant_cost);
        w.int(self.repair_cost);
        w.byte(u8::from(self.has_item) | u8::from(self.changed) << 1 | u8::from(self.repair_allowed) << 2);
        w.int(self.rows.len() as i32);
        for r in &self.rows {
            w.text(&r.id);
            w.int(r.level as i32);
            w.int(r.data_max as i32);
            w.int(r.allowed as i32);
            w.int(r.required);
            w.byte(u8::from(r.buttons.0) | u8::from(r.buttons.1) << 1 | u8::from(r.buttons.2) << 2 | u8::from(r.buttons.3) << 3);
            w.int(r.incompatible.len() as i32);
            for id in &r.incompatible {
                w.text(id);
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Option<Self> {
        let mut r = Reader(b);
        let mut v = View {
            power: r.int()?,
            max_power: r.int()?,
            enchant_cost: r.int()?,
            repair_cost: r.int()?,
            ..Default::default()
        };
        let flags = r.byte()?;
        v.has_item = flags & 1 != 0;
        v.changed = flags & 2 != 0;
        v.repair_allowed = flags & 4 != 0;
        let n = r.int()?;
        if !(0..=4096).contains(&n) {
            return None;
        }
        for _ in 0..n {
            let id = r.text()?;
            let level = r.int()? as u32;
            let data_max = r.int()? as u32;
            let allowed = r.int()? as u32;
            let required = r.int()?;
            let f = r.byte()?;
            let k = r.int()?;
            if !(0..=4096).contains(&k) {
                return None;
            }
            let mut incompatible = Vec::new();
            for _ in 0..k {
                incompatible.push(r.text()?);
            }
            v.rows.push(Row {
                id,
                level,
                data_max,
                allowed,
                required,
                incompatible,
                buttons: (f & 1 != 0, f & 2 != 0, f & 4 != 0, f & 8 != 0),
            });
        }
        r.0.is_empty().then_some(v)
    }
}

#[derive(Default)]
struct Writer(Vec<u8>);
impl Writer {
    fn byte(&mut self, b: u8) {
        self.0.push(b);
    }
    fn int(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn text(&mut self, s: &str) {
        self.int(s.len() as i32);
        self.0.extend_from_slice(s.as_bytes());
    }
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn byte(&mut self) -> Option<u8> {
        let (b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(*b)
    }
    fn int(&mut self) -> Option<i32> {
        if self.0.len() < 4 {
            return None;
        }
        let (a, rest) = self.0.split_at(4);
        self.0 = rest;
        Some(i32::from_le_bytes([a[0], a[1], a[2], a[3]]))
    }
    fn text(&mut self) -> Option<String> {
        let n = self.int()?;
        if n < 0 || n as usize > self.0.len() {
            return None;
        }
        let (a, rest) = self.0.split_at(n as usize);
        self.0 = rest;
        String::from_utf8(a.to_vec()).ok()
    }
}

/// `appendHoverText` of the infuser items: "Choose (, modify and remove) enchantments for your gear."
/// and, with repairing, " Repair your gear with levels." (translation keys).
pub fn block_tooltip_keys(cfg: &InfuserConfig) -> (&'static str, bool) {
    let key = if cfg.modifying == Modifiable::Unenchanted {
        "block.enchantinginfuser.description.choose"
    } else {
        "block.enchantinginfuser.description.chooseAndModify"
    };
    (key, cfg.repairing != Repairs::Nothing)
}

/// The one-slot rule (`Slot.getMaxStackSize` = 1) after the game's chest menu ran a click on the
/// infuser's menu: `before`/`after` are the menu slots (0 the infuser's, 1..9 stand-ins, then the
/// player's 36), `carried_after` the cursor. The extra items go back to the cursor (a click that
/// placed them), to the slot a Shift-click took them from, or are refused (None: the click is dropped).
pub fn settle_one<S: Stack>(
    before: &[Option<S>],
    after: &[Option<S>],
    carried_after: &Option<S>,
    quick_move: Option<usize>,
) -> Option<(Vec<Option<S>>, Option<S>)> {
    let mut out = after.to_vec();
    let mut carried = carried_after.clone();
    let mut spilled: Vec<S> = out[1..9].iter_mut().filter_map(Option::take).collect();
    if let Some(s) = out[0].as_mut()
        && s.count() > 1
    {
        let mut rest = s.clone();
        rest.set_count(s.count() - 1);
        s.set_count(1);
        spilled.push(rest);
    }
    let _ = before;
    for s in spilled {
        if let Some(src) = quick_move.filter(|&q| q >= 9 && q < out.len()) {
            match &mut out[src] {
                None => {
                    out[src] = Some(s);
                    continue;
                }
                Some(back) if back.same_kind(&s) => {
                    back.set_count(back.count() + s.count());
                    continue;
                }
                Some(_) => return None,
            }
        }
        match &mut carried {
            None => carried = Some(s),
            Some(c) if c.same_kind(&s) => c.set_count(c.count() + s.count()),
            Some(_) => return None,
        }
    }
    Some((out, carried))
}

/// A stack as `settle_one` handles it.
pub trait Stack: Clone {
    fn count(&self) -> u32;
    fn set_count(&mut self, count: u32);
    fn same_kind(&self, other: &Self) -> bool;
}
