// Enchanting Infuser inside Horizon's game (nightly 142388e1c). The rules are the engine below (src/lib.rs).
//
// The two infusers are carried blocks with the template `minecraft:enchanting_table` (its shape, light 7,
// floating book). Their one slot is Horizon's own `BlockEntity::Chest(vec![None; 1])` (ironchest's
// FUNDATIE.md): saved with the chunk, spilled on break, seen by hoppers (only an empty slot takes an
// item, only an enchanted item comes out, as `InfuserBlockEntity`). The menu (`InfuserMenu` +
// `InfuserScreen`): the game's chest menu engine over the one slot (like the dirt chest of Iron Chests,
// one item at most), the list of enchantments, the enchant and repair buttons and the enchanting power
// drawn over the mod's own screen texture. Single player's window is a guest of the integrated server:
// it gets the menu as a forja menu (`enchantinginfuser:infuser`), the server runs the clicks as the guest,
// the list comes over as a forja section (`engine::View`).
#[allow(unused_imports)]
use super::{DragView, Game, Result, Screen, lan::LanHost};
use horizon_client::{
    containers::BlockEntity,
    hud::{HudBatch, rgba},
    items::ItemStack,
    menu::{Env, Input, Kind, Menu},
};
use std::cell::RefCell;
use std::collections::HashMap;

#[allow(dead_code)]
mod engine {
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

/// What the screen shows (the server's session, as a forja menu section for a guest's window).
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

}

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

const MENU_KIND: &str = "enchantinginfuser:infuser";
/// The screen texture in the GUI atlas (`textures/gui/container/enchanting_infuser.png`, carried under
/// `textures/block/` so the GUI atlas packs it: FUNDATIE.md section 3).
const TEXTURE: &str = "enchantinginfuser:block/gui_enchanting_infuser";
/// The session key of a host playing in the frame (guests: their connection).
const HOST: u32 = u32::MAX;

impl engine::Stack for ItemStack {
    fn count(&self) -> u32 {
        self.count
    }
    fn set_count(&mut self, count: u32) {
        self.count = count;
    }
    fn same_kind(&self, other: &Self) -> bool {
        self.item == other.item && self.damage == other.damage && self.extra == other.extra
    }
}

/// One player's open infuser menu (the server's `InfuserMenu`).
struct Open {
    pos: [i32; 3],
    /// The slot's item the session was made for (`slotsChanged` when it differs).
    item: Option<ItemStack>,
    session: engine::Session,
}

#[derive(Default)]
struct State {
    /// By player: HOST or a guest's connection.
    open: HashMap<u32, Open>,
    /// The window's own scrollbar position (0..1) and dragging.
    scroll: f32,
    dragging: bool,
    /// `InfuserScreen.enchantmentSeed` (`new Random().nextInt()` per screen).
    seed: i32,
    /// The infuser the window last drew (a new one: the screen starts over).
    shown: Option<[i32; 3]>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The game's enchantments and items, as the rules ask (26.3 data and tags).
struct Catalog<'a> {
    e: &'a horizon_client::enchant::Enchantments,
    items: &'a horizon_client::items::Items,
}
impl engine::Catalog for Catalog<'_> {
    fn enchantments(&self) -> Vec<engine::Ench> {
        self.e
            .list
            .iter()
            .map(|x| engine::Ench {
                id: x.id.to_string(),
                rarity: engine::Rarity::from_weight(x.weight),
                max_level: x.max_level,
                min_cost_base: x.min_cost.base,
                min_cost_per_level: x.min_cost.per_level,
                curse: self.e.in_tag("minecraft:curse", &x.id),
                treasure: self.e.in_tag("minecraft:treasure", &x.id),
                tradeable: self.e.in_tag("minecraft:tradeable", &x.id),
                discoverable: self.e.in_tag("minecraft:on_random_loot", &x.id),
            })
            .collect()
    }
    fn compatible(&self, a: &str, b: &str) -> bool {
        self.e.compatible(a, b)
    }
    fn applies(&self, ench: &str, item: &str, anvil: bool) -> bool {
        if anvil {
            self.e.can_enchant(self.items, ench, item)
        } else {
            self.e.get(ench).is_some_and(|x| self.e.is_primary(self.items, x, item))
        }
    }
}

impl Game<'_> {
    // ---- blocks and the slot ----

    /// The infuser at `p`: its block name.
    fn infuser_at(&self, p: [i32; 3]) -> Option<&'static str> {
        let name = kit_blocks::name_at(self, p)?;
        [engine::NORMAL, engine::ADVANCED].into_iter().find(|n| *n == &*name)
    }

    fn infuser_ensure_slot(&mut self, p: [i32; 3]) {
        let key = (p[0], p[1], p[2]);
        match self.world.block_entities.get_mut(&key) {
            Some(BlockEntity::Chest(items)) => {
                if items.is_empty() {
                    items.push(None);
                }
            }
            Some(_) => {}
            None => {
                self.world.block_entities.insert(key, BlockEntity::Chest(vec![None]));
                self.world.modified.insert((key.0 >> 4, key.2 >> 4));
            }
        }
    }

    fn infuser_slot_item(&self, p: [i32; 3]) -> Option<ItemStack> {
        match self.world.block_entities.get(&(p[0], p[1], p[2])) {
            Some(BlockEntity::Chest(items)) => items.first().cloned().flatten(),
            _ => None,
        }
    }

    fn infuser_item(&self, s: &ItemStack) -> engine::Item {
        let info = self.res.items().get(&s.item);
        let tag = |t: &str| kit_blocks::item_in_tag(self, t, &s.item);
        let tool_or_armor = [
            "minecraft:pickaxes", "minecraft:axes", "minecraft:shovels", "minecraft:hoes", "minecraft:swords",
            "minecraft:head_armor", "minecraft:chest_armor", "minecraft:leg_armor", "minecraft:foot_armor",
        ]
        .iter()
        .any(|t| tag(t));
        let enchantments = if &*s.item == "minecraft:enchanted_book" { s.stored_enchantments() } else { s.enchantments() };
        engine::Item {
            id: s.item.to_string(),
            count: s.count,
            max_stack: info.map_or(64, |i| i.max_stack),
            max_damage: info.map_or(0, |i| i.max_damage),
            damage: s.damage,
            enchantments: enchantments.iter().map(|(id, l)| (id.to_string(), *l)).collect(),
            tool_or_armor,
        }
    }

    /// `InfuserMenu.getAvailablePower` around the infuser at `p`.
    fn infuser_power(&self, cfg: &engine::InfuserConfig, p: [i32; 3]) -> i32 {
        engine::power(cfg, |o| {
            let at = [p[0] + o[0], p[1] + o[1], p[2] + o[2]];
            let between = [p[0] + o[0] / 2, p[1] + o[1], p[2] + o[2] / 2];
            let state = kit_blocks::state_at(self, at);
            let name = kit_blocks::name_of(self, state);
            let bonus = if kit_blocks::block_in_tag(self, "minecraft:enchantment_power_provider", &name) { 1.0 } else { 0.0 };
            let chiseled_books = if &*name == "minecraft:chiseled_bookshelf"
                && kit_blocks::prop(self, state, "facing").is_some_and(|f| engine::chiseled_faces_infuser(o, &f))
            {
                (0..6).filter(|i| kit_blocks::prop(self, state, &format!("slot_{i}_occupied")).as_deref() == Some("true")).count() as i32
            } else {
                0
            };
            let full = self.res.facts().get(kit_blocks::state_at(self, between)).full_collision;
            engine::Spot { bonus, chiseled_books, between_full: full }
        })
    }

    /// The session of player `key` at the infuser `p`, made again when the slot's item changed
    /// (`slotsChanged`); runs `f` on it.
    fn infuser_with<R>(&self, key: u32, p: [i32; 3], f: impl FnOnce(&mut engine::Session) -> R) -> Option<R> {
        let block = self.infuser_at(p)?;
        let cfg = engine::config_of(block)?;
        let item = self.infuser_slot_item(p);
        let fresh = STATE.with(|s| {
            s.borrow().open.get(&key).is_none_or(|o| o.pos != p || o.item != item || o.session.cfg != *cfg)
        });
        if fresh {
            let catalog = Catalog { e: &self.res.stations().enchantments, items: self.res.items() };
            let power = self.infuser_power(cfg, p);
            let session = engine::Session::new(cfg, &catalog, item.as_ref().map(|s| self.infuser_item(s)), power);
            STATE.with(|s| s.borrow_mut().open.insert(key, Open { pos: p, item: item.clone(), session }));
        }
        STATE.with(|s| s.borrow_mut().open.get_mut(&key).map(|o| f(&mut o.session)))
    }

    /// The session key of whoever acts here (a guest in his turn, else the host).
    fn infuser_key(&self) -> u32 {
        self.lan_acting.unwrap_or(HOST)
    }

    /// The enchantment's name (`Component.translatable(getDescriptionId())`).
    fn infuser_name(&self, id: &str) -> String {
        let e = &self.res.stations().enchantments;
        match e.get(id) {
            Some(x) => self.translate(&x.description_key, &x.description),
            None => id.to_owned(),
        }
    }

    fn infuser_view(&self, key: u32, p: [i32; 3]) -> Option<engine::View> {
        self.infuser_with(key, p, |s| {
            let order = s.sorted(|id| self.infuser_name(id));
            engine::View::of(s, &order)
        })
    }

    // ---- placing, opening, the menu kind ----

    /// HEAD `advancements::trigger_block` ("minecraft:placed_block", BlockItem.place's end): the slot, and
    /// the item's name on it.
    pub(super) fn infuser_placed(&mut self, trigger: &str, pos: [i32; 3], stack: &ItemStack) {
        if trigger != "minecraft:placed_block" || self.infuser_at(pos).is_none() {
            return;
        }
        let had = self.world.block_entities.contains_key(&(pos[0], pos[1], pos[2]));
        self.infuser_ensure_slot(pos);
        if !had {
            let game_master = self.can_use_game_master_blocks();
            self.apply_placed_item(pos, stack, game_master);
            self.infuser_ensure_slot(pos);
        }
    }

    /// HEAD cancellable `stations::open_station`: `InfuserBlock.use` (open_menu reaches it for a block
    /// the game's block entities do not know). A guest's turn: the LAN code opens it for him afterwards
    /// (`lan_open_container`).
    pub(super) fn infuser_open(&mut self, pos: [i32; 3], name: &str) -> bool {
        if engine::config_of(name).is_none() {
            return false;
        }
        infuser_register_menu();
        self.infuser_ensure_slot(pos);
        self.container = Some((pos[0], pos[1], pos[2]));
        let seed = {
            use horizon_core::random::RandomSource;
            self.world.random.next_int()
        };
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.scroll = 0.0;
            s.dragging = false;
            s.seed = seed;
            s.shown = Some(pos);
        });
        let key = self.infuser_key();
        STATE.with(|s| s.borrow_mut().open.remove(&key));
        true
    }

    /// HEAD cancellable `lan_actions::lan_open_container`: a guest opened an infuser (single player's
    /// window is one). With forja on his client: the infuser's menu as a forja menu; else nothing (the
    /// protocol has no menu of this shape).
    pub(super) fn infuser_lan_open(&mut self, host: &mut LanHost, conn: u32, pos: (i32, i32, i32)) -> Option<bool> {
        let p = [pos.0, pos.1, pos.2];
        self.infuser_at(p)?;
        infuser_register_menu();
        self.infuser_ensure_slot(p);
        STATE.with(|s| s.borrow_mut().open.remove(&conn));
        if super::forja::guest_has_forja(conn) {
            return Some(self.forja_menu_open(host, conn, MENU_KIND, p));
        }
        host.hub.send_to(
            conn,
            &horizon_protocol::packets::play::Clientbound::SystemChat(horizon_protocol::packets::play::SystemChat {
                content: horizon_protocol::text::literal("Enchanting Infuser: this client cannot show its screen"),
                overlay: true,
            }),
        );
        Some(false)
    }

    /// The infuser whose menu is open here: on a server's client (single player's window) the forja menu.
    fn infuser_open_menu(&self) -> Option<[i32; 3]> {
        if self.remote.is_some() {
            let (kind, p) = super::forja::guest_menu(self)?;
            return (kind == MENU_KIND).then_some(p);
        }
        if self.station.is_some() || self.screen != Screen::Inventory {
            return None;
        }
        let key = self.container?;
        let p = [key.0, key.1, key.2];
        self.infuser_at(p).map(|_| p)
    }

    /// The config of the open infuser.
    fn infuser_open_config(&self) -> Option<(&'static engine::InfuserConfig, [i32; 3])> {
        let p = self.infuser_open_menu()?;
        Some((engine::config_of(self.infuser_at(p)?)?, p))
    }

    // ---- the slot clicks (the chest menu engine over one slot) ----

    fn infuser_session_menu(
        &mut self,
        quick_move_from: Option<usize>,
        run: impl FnOnce(&mut Menu, &Env) -> Option<bool>,
    ) -> bool {
        // Menu slots: 0 the infuser's, 1..9 stand-ins, 9..36 inventory, 36..45 hotbar.
        let to_game = |m: usize| -> Option<usize> {
            match m {
                0 => Some(engine::CONTAINER_SLOT),
                1..9 => None,
                9..36 => Some(m),
                36..45 => Some(m - 36),
                _ => None,
            }
        };
        let total = 45;
        let before: Vec<Option<ItemStack>> = (0..total).map(|i| to_game(i).and_then(|g| self.slot_mut(g).clone())).collect();
        let carried = self.carried.clone();
        let offhand = self.offhand.clone();
        let (outcome, after, carried, offhand, dropped, sounds, equipped) = {
            let stations = self.res.stations();
            let env = Env {
                items: self.res.items(),
                recipes: self.res.recipes(),
                brewing: &stations.brewing,
                banners: &stations.banners,
                station_recipes: &stations.recipes,
                enchantments: &stations.enchantments,
                names: &self.pack.names,
                trade: None,
                maps: None,
            };
            let mut menu = Menu::new(Kind::Chest(1));
            menu.creative = self.creative;
            menu.selected = self.selected;
            menu.levels = self.experience.level;
            for (i, stack) in before.iter().enumerate() {
                menu.set_slot(&env, i, stack.clone());
            }
            menu.set_player_item(40, offhand);
            menu.settle(&env);
            menu.carried = carried;
            menu.drops.clear();
            let outcome = run(&mut menu, &env);
            let after: Vec<Option<ItemStack>> = (0..total).map(|i| menu.slot(i).cloned()).collect();
            (
                outcome,
                after,
                menu.carried.take(),
                menu.player_item(40).cloned(),
                std::mem::take(&mut menu.dropped),
                std::mem::take(&mut menu.player_sounds),
                std::mem::take(&mut menu.equipped),
            )
        };
        match outcome {
            None => return false,
            Some(false) => return true,
            Some(true) => {}
        }
        // Slot.getMaxStackSize() = 1.
        let Some((after, carried)) = engine::settle_one(&before, &after, &carried, quick_move_from) else {
            return true;
        };
        for sound in sounds {
            self.player_menu_sound(sound);
        }
        for (i, stack) in after.into_iter().enumerate() {
            if let Some(g) = to_game(i) {
                *self.slot_mut(g) = stack;
            }
        }
        self.carried = carried;
        self.offhand = offhand;
        for stack in dropped {
            self.player_drop(stack);
        }
        for (slot, stack) in equipped {
            self.player_equipped(slot, stack.as_ref());
        }
        self.apply_equipment();
        if let Some(key) = self.container {
            self.world.modified.insert((key.0 >> 4, key.2 >> 4));
        }
        true
    }

    fn infuser_to_menu(game_slot: usize) -> Option<usize> {
        match game_slot {
            engine::CONTAINER_SLOT => Some(0),
            9..36 => Some(game_slot),
            0..9 => Some(36 + game_slot),
            _ => None,
        }
    }

    /// HEAD cancellable `Game::menu_ops` on an open infuser (the host's window, or a guest's clicks run
    /// on the server): `None` for every other menu.
    pub(super) fn infuser_menu_ops(&mut self, ops: &[(Option<usize>, i32, Input)]) -> Option<bool> {
        if self.remote.is_some() {
            return None;
        }
        self.infuser_open_menu()?;
        let quick_move = match ops {
            [(Some(slot), _, Input::QuickMove)] => Self::infuser_to_menu(*slot),
            _ => None,
        };
        let ops = ops.to_vec();
        let handled = self.infuser_session_menu(quick_move, |menu, env| {
            for (slot, button, input) in ops {
                let menu_slot = match slot {
                    Some(g) => Self::infuser_to_menu(g)? as i32,
                    None => -999,
                };
                if menu.click(env, menu_slot, button, input).is_err() {
                    return Some(false);
                }
            }
            Some(true)
        });
        Some(handled)
    }

    /// HEAD cancellable `drag_view`: the quick craft preview on the infuser's slots.
    pub(super) fn infuser_drag_view(&mut self) -> Option<Option<DragView>> {
        if self.remote.is_some() {
            return None;
        }
        self.infuser_open_menu()?;
        let Some((ty, slots)) = self.inv_drag.clone() else {
            return Some(None);
        };
        let mut view = None;
        self.infuser_session_menu(None, |menu, env| {
            let menu_slots: Option<Vec<usize>> = slots.iter().map(|&g| Self::infuser_to_menu(g)).collect();
            view = Some(match menu.quick_craft_preview(env, ty, &menu_slots?) {
                Some((shown, left)) => DragView { slots: slots.iter().copied().zip(shown).collect(), left: Some(left) },
                None => DragView { slots: slots.iter().map(|&g| (g, None)).collect(), left: None },
            });
            Some(false)
        });
        Some(view)
    }

    /// HEAD cancellable `quick_craft_accepts`.
    pub(super) fn infuser_quick_craft_accepts(&mut self, slot: usize, ty: i32, dragged: usize) -> Option<bool> {
        if self.remote.is_some() {
            return None;
        }
        self.infuser_open_menu()?;
        let mut accepts = false;
        self.infuser_session_menu(None, |menu, env| {
            accepts = Self::infuser_to_menu(slot).is_some_and(|s| menu.quick_craft_accepts(env, s, ty, dragged));
            Some(false)
        });
        Some(accepts)
    }

    // ---- hoppers (InfuserBlockEntity as a WorldlyContainer) ----

    /// `canPlaceItemThroughFace`: only into an empty slot. `None` = not an infuser.
    pub(super) fn infuser_move_in(&mut self, p: [i32; 3], stack: &ItemStack) -> Option<Option<ItemStack>> {
        self.infuser_at(p)?;
        self.infuser_ensure_slot(p);
        if self.infuser_slot_item(p).is_some() {
            return Some(Some(stack.clone()));
        }
        if let Some(BlockEntity::Chest(items)) = self.world.block_entities.get_mut(&(p[0], p[1], p[2])) {
            items[0] = Some(stack.clone());
        }
        self.world.modified.insert((p[0] >> 4, p[2] >> 4));
        Some(None)
    }

    /// `canTakeItemThroughFace`: only an enchanted item or an enchanted book comes out.
    pub(super) fn infuser_protects(&self, p: [i32; 3]) -> bool {
        if self.infuser_at(p).is_none() {
            return false;
        }
        !self.infuser_slot_item(p).is_some_and(|s| !s.enchantments().is_empty() || &*s.item == "minecraft:enchanted_book")
    }

    // ---- the buttons (clickEnchantButton, clickRepairButton, the level buttons) ----

    /// A press on the screen at the cursor, run where the menu lives (the host, or the server as the
    /// guest). `scroll` = the window's first list row.
    fn infuser_press(&mut self, scroll: usize, shift: bool) -> bool {
        let Some((cfg, p)) = self.infuser_open_config() else { return false };
        let Some(batch) = super::forja::hud_batch(self) else { return false };
        let (ox, oy) = batch.centered_origin(engine::screen::WIDTH, engine::screen::HEIGHT);
        let (mx, my) = (self.cursor.0 - ox, self.cursor.1 - oy);
        let key = self.infuser_key();
        let Some(view) = self.infuser_view(key, p) else { return false };
        let Some(widget) = engine::widget_at(cfg, mx, my, scroll, view.rows.len(), |i| view.rows[i].buttons()) else {
            return false;
        };
        let levels = self.experience.level;
        let creative = self.creative;
        match widget {
            engine::Widget::Scrollbar => false,
            engine::Widget::Level { entry, increase } => {
                let id = view.rows[entry].id.clone();
                let done = self.infuser_with(key, p, |s| {
                    let i = s.enchants.iter().position(|e| e.id == id)?;
                    Some(s.click_level_repeat(i, increase, shift))
                });
                if done.flatten().unwrap_or(false) {
                    self.audio.play_button_click();
                }
                true
            }
            engine::Widget::Enchant => {
                let ready = self.infuser_with(key, p, |s| s.can_enchant(levels, creative)).unwrap_or(false);
                if ready {
                    self.audio.play_button_click();
                    self.infuser_enchant(key, p);
                }
                true
            }
            engine::Widget::Repair => {
                let ready = self.infuser_with(key, p, |s| s.can_repair(levels, creative)).unwrap_or(false);
                if ready {
                    self.audio.play_button_click();
                    self.infuser_repair(key, p, cfg);
                }
                true
            }
        }
    }

    /// `clickEnchantButton`.
    fn infuser_enchant(&mut self, key: u32, p: [i32; 3]) {
        let creative = self.creative;
        let levels = self.experience.level;
        let got = self
            .infuser_with(key, p, |s| {
                if !s.can_enchant(levels, creative) {
                    return None;
                }
                let result = s.result()?;
                Some((s.enchant_cost, s.clone(), result))
            })
            .flatten();
        let Some((cost, xp_back, result)) = got else {
            return;
        };
        let Some(stack) = self.infuser_slot_item(p) else { return };
        if cost < 0 {
            let random = &mut self.world.random;
            use horizon_core::random::RandomSource;
            let xp = xp_back.experience_delta(|n| random.next_int_bound(n).unwrap_or(0));
            // ExperienceOrb.award(level, Vec3.atCenterOf(pos), xp): orbs of the largest sizes.
            let at = [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5];
            let mut amount = xp.max(0) as u32;
            while amount > 0 {
                let value = horizon_world::level::removal::orb_value(amount);
                amount -= value;
                let merge_id = self.world.random.next_int_bound(40).unwrap_or(0);
                self.award_orb(at, value, merge_id);
            }
        } else if !creative {
            self.give_levels(-cost);
        }
        let (id, enchantments) = result;
        let mut out = stack.clone();
        if *out.item != *id {
            out.item = std::sync::Arc::from(id.as_str());
        }
        let levels: Vec<(std::sync::Arc<str>, u32)> =
            enchantments.iter().map(|(e, l)| (std::sync::Arc::from(e.as_str()), *l)).collect();
        if &*out.item == "minecraft:enchanted_book" {
            out.data_mut().enchantments.clear();
            out.data_mut().stored_enchantments = levels;
        } else {
            if let Some(d) = out.extra.as_mut() {
                d.stored_enchantments.clear();
            }
            out.data_mut().enchantments = levels;
        }
        out.tidy();
        if let Some(BlockEntity::Chest(items)) = self.world.block_entities.get_mut(&(p[0], p[1], p[2])) {
            items[0] = Some(out.clone());
        }
        self.world.modified.insert((p[0] >> 4, p[2] >> 4));
        self.stats.custom("enchant_item", 1);
        self.trigger_enchanted_item(&out, cost);
        use horizon_core::random::RandomSource;
        let pitch = self.world.random.next_float() * 0.1 + 0.9;
        kit_sound::at_block(self, "minecraft:block.enchantment_table.use", p, 1.0, pitch);
        // slotsChanged: the session follows the new item at the next look.
    }

    /// `clickRepairButton`: the cost counted again from the item, levels paid, the item mended.
    fn infuser_repair(&mut self, _key: u32, p: [i32; 3], cfg: &engine::InfuserConfig) {
        let Some(stack) = self.infuser_slot_item(p) else { return };
        let item = self.infuser_item(&stack);
        if !engine::repair_allowed(cfg, Some(&item)) {
            return;
        }
        let cost = engine::repair_cost(cfg, Some(&item));
        if self.experience.level < cost && !self.creative {
            return;
        }
        if !self.creative {
            self.give_levels(-cost);
        }
        let mut out = stack;
        out.damage = 0;
        if let Some(BlockEntity::Chest(items)) = self.world.block_entities.get_mut(&(p[0], p[1], p[2])) {
            items[0] = Some(out);
        }
        self.world.modified.insert((p[0] >> 4, p[2] >> 4));
        // LevelEvent.SOUND_ANVIL_USED
        kit_sound::level_event(self, 1030, p, 0);
    }

    // ---- the window: mouse, wheel ----

    /// HEAD cancellable `tweaks::sort_mouse_press` (every press first): the infuser's widgets.
    pub(super) fn infuser_mouse(&mut self, button: winit::event::MouseButton) -> bool {
        let Some((cfg, _)) = self.infuser_open_config() else { return false };
        if button != winit::event::MouseButton::Left {
            return false;
        }
        let Some(batch) = super::forja::hud_batch(self) else { return false };
        let (ox, oy) = batch.centered_origin(engine::screen::WIDTH, engine::screen::HEIGHT);
        let (mx, my) = (self.cursor.0 - ox, self.cursor.1 - oy);
        let Some(view) = self.infuser_window_view() else { return false };
        let (scroll_pos, _) = STATE.with(|s| (s.borrow().scroll, s.borrow().dragging));
        let scroll = engine::scroll_rows(scroll_pos, view.rows.len());
        let hit = engine::widget_at(cfg, mx, my, scroll, view.rows.len(), |i| view.rows[i].buttons());
        match hit {
            None => false,
            Some(engine::Widget::Scrollbar) => {
                if view.rows.len() > engine::screen::ROWS {
                    STATE.with(|s| {
                        let mut s = s.borrow_mut();
                        s.dragging = true;
                        s.scroll = engine::bar_position(my);
                    });
                }
                true
            }
            Some(w) => {
                if self.remote.is_some() {
                    // The server runs the press as this guest; here only the button's click sound.
                    let (levels, creative) = (self.experience.level, self.creative);
                    let active = match w {
                        engine::Widget::Level { entry, increase } => {
                            let b = view.rows[entry].buttons();
                            if increase { b.incr_active } else { b.decr_active }
                        }
                        engine::Widget::Enchant => view.can_enchant(levels, creative),
                        engine::Widget::Repair => view.can_repair(levels, creative),
                        engine::Widget::Scrollbar => false,
                    };
                    let input = super::forja::mouse_input(self, button, vec![scroll.min(255) as u8]);
                    super::forja::guest_send(self, input);
                    if active {
                        self.audio.play_button_click();
                    }
                    return true;
                }
                let shift = self.has_shift_down();
                self.infuser_press(scroll, shift)
            }
        }
    }

    /// HEAD `inventory_drag_move`: the scrollbar follows the cursor while held.
    pub(super) fn infuser_drag(&mut self) {
        if !STATE.with(|s| s.borrow().dragging) {
            return;
        }
        if self.infuser_open_menu().is_none() {
            STATE.with(|s| s.borrow_mut().dragging = false);
            return;
        }
        let Some(batch) = super::forja::hud_batch(self) else { return };
        let (_, oy) = batch.centered_origin(engine::screen::WIDTH, engine::screen::HEIGHT);
        let my = self.cursor.1 - oy;
        STATE.with(|s| s.borrow_mut().scroll = engine::bar_position(my));
    }

    /// HEAD `inventory_mouse_release`: the scrollbar is let go.
    pub(super) fn infuser_release(&mut self) {
        STATE.with(|s| s.borrow_mut().dragging = false);
    }

    /// HEAD cancellable `tweaks::tweak_scroll`: the wheel scrolls the list (`mouseScrolled`).
    pub(super) fn infuser_wheel(&mut self, delta: f32) -> bool {
        if self.infuser_open_menu().is_none() {
            return false;
        }
        let Some(view) = self.infuser_window_view() else { return false };
        if view.rows.len() <= engine::screen::ROWS {
            return false;
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.scroll = engine::wheel(s.scroll, delta.signum(), view.rows.len());
        });
        true
    }

    /// The list the window shows: the host's own session, or the server's section on a guest's window.
    fn infuser_window_view(&self) -> Option<engine::View> {
        let p = self.infuser_open_menu()?;
        if self.remote.is_some() {
            let (_, sections) = super::forja::guest_sections(self)?;
            return engine::View::decode(sections.first()?);
        }
        self.infuser_view(HOST, p)
    }

    // ---- the screen ----

    pub(super) fn infuser_origin(&self, batch: &HudBatch) -> Option<(f32, f32)> {
        self.infuser_open_menu()?;
        Some(batch.centered_origin(engine::screen::WIDTH, engine::screen::HEIGHT))
    }
    pub(super) fn infuser_screen_height(&self) -> Option<f32> {
        self.infuser_open_menu().map(|_| engine::screen::HEIGHT)
    }
    pub(super) fn infuser_image(&self, batch: &HudBatch) -> Option<horizon_client::container_screen::Image> {
        self.infuser_open_menu()?;
        let (ox, oy) = batch.centered_origin(engine::screen::WIDTH, engine::screen::HEIGHT);
        Some(horizon_client::container_screen::Image {
            left: ox as i32,
            top: oy as i32,
            width: engine::screen::WIDTH as i32,
            height: engine::screen::HEIGHT as i32,
        })
    }
    pub(super) fn infuser_slot_position(&self, i: usize) -> Option<Option<(f32, f32)>> {
        let (cfg, _) = self.infuser_open_config()?;
        Some(engine::slot_position(cfg, i))
    }

    /// A part of the screen texture (`guiGraphics.blit(INFUSER_LOCATION, x, y, u, v, w, h)`).
    fn infuser_blit(&self, batch: &mut HudBatch, x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
        let atlas = &self.gui.atlas;
        let Some(mut r) = atlas.sprites.get(TEXTURE).copied() else {
            return;
        };
        let t = engine::screen::TEXTURE_SIZE;
        let (du, dv) = (r.u1 - r.u0, r.v1 - r.v0);
        let (u0, v0) = (r.u0, r.v0);
        r.u0 = u0 + du * u / t;
        r.u1 = u0 + du * (u + w) / t;
        r.v0 = v0 + dv * v / t;
        r.v1 = v0 + dv * (v + h) / t;
        batch.rect_uv(x, y, w, h, r, u32::MAX);
    }

    /// HEAD cancellable `machines::draw_machine_screen`: `InfuserScreen.renderBg` and `renderLabels`.
    pub(super) fn infuser_draw_screen(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some((cfg, p)) = self.infuser_open_config() else { return false };
        use engine::screen::*;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.shown != Some(p) {
                s.shown = Some(p);
                s.scroll = 0.0;
                s.dragging = false;
                s.seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as i32);
            }
        });
        let atlas = &self.gui.atlas;
        if atlas.sprites.contains_key(TEXTURE) {
            self.infuser_blit(batch, ox, oy, 0.0, 0.0, WIDTH, HEIGHT);
        } else {
            batch.fill(atlas, ox, oy, WIDTH, HEIGHT, rgba(198, 198, 198, 255));
        }
        // The armour and off hand slots of the original's menu are not here: their frames are covered.
        let panel = rgba(198, 198, 198, 255);
        for (x, y) in [(7.0, 102.0), (7.0, 120.0), (195.0, 102.0), (195.0, 120.0), (7.0, 160.0)] {
            batch.fill(atlas, ox + x, oy + y, 18.0, 18.0, panel);
        }
        // The slot's frame, lower when there is no repair button.
        let slot_y = if cfg.repairing != engine::Repairs::Nothing { 23.0 } else { 34.0 };
        self.infuser_blit(batch, ox + 7.0, oy + slot_y - 1.0, 162.0, 185.0, 18.0, 18.0);
        let view = self.infuser_window_view().unwrap_or_default();
        let scroll_pos = STATE.with(|s| s.borrow().scroll);
        let can_scroll = view.rows.len() > ROWS;
        let range = BAR_H + 4.0 - 18.0;
        self.infuser_blit(batch, ox + BAR_X - 2.0, oy + BAR_Y - 2.0 + (range * scroll_pos).floor(), 220.0, if can_scroll { 72.0 } else { 54.0 }, 18.0, 18.0);
        let grey = rgba(64, 64, 64, 255);
        let title = self.container_title(self.translate(&format!("block.{}", self.infuser_at(p).unwrap_or(engine::NORMAL).replace(':', ".")), "Enchanting Infuser"));
        batch.text(atlas, &title, ox + TITLE_LABEL.0, oy + TITLE_LABEL.1, grey, false);
        let inventory = self.translate("container.inventory", "Inventory");
        batch.text(atlas, &inventory, ox + INVENTORY_LABEL.0, oy + INVENTORY_LABEL.1, grey, false);
        true
    }

    /// HEAD `draw_carried` (over the slots, under the carried stack): the list, the buttons, the costs,
    /// the enchanting power and the tooltips (`InfuserScreen.render`).
    pub(super) fn infuser_draw_over(&self, batch: &mut HudBatch) {
        let Some((cfg, _p)) = self.infuser_open_config() else { return };
        use engine::screen::*;
        let Some(view) = self.infuser_window_view() else { return };
        let (ox, oy) = batch.centered_origin(WIDTH, HEIGHT);
        let (mx, my) = (self.cursor.0, self.cursor.1);
        let atlas = &self.gui.atlas;
        let levels = self.experience.level;
        let creative = self.creative;
        let scroll = engine::scroll_rows(STATE.with(|s| s.borrow().scroll), view.rows.len());
        let shift = self.has_shift_down();
        let mut tip: Option<Vec<(String, u32)>> = None;
        let mut insufficient = false;
        let white = u32::MAX;
        let gray = rgba(170, 170, 170, 255);
        let power_line = |cur: String| {
            let label = self.translate("gui.enchantinginfuser.tooltip.enchanting_power", "Enchanting Power");
            self.translate("gui.enchantinginfuser.tooltip.current_enchanting_power", "Current %s: %s / %s")
                .replacen("%s", &label, 1)
                .replacen("%s", &cur, 1)
        };
        // The rows.
        for row in 0..ROWS.min(view.rows.len()) {
            let Some(r) = view.rows.get(scroll + row) else { break };
            let (x, y) = (ox + LIST_X, oy + LIST_Y + ITEM_H * row as f32);
            let incompatible = !r.incompatible.is_empty();
            let image = if incompatible || r.obfuscated() { 0.0 } else if r.level > 0 { 2.0 } else { 1.0 };
            self.infuser_blit(batch, x + 18.0, y, 0.0, 185.0 + image * 18.0, 126.0, 18.0);
            let name = if r.obfuscated() {
                String::new()
            } else if r.level > 0 {
                format!("{} {}", self.infuser_name(&r.id), self.translate(&format!("enchantment.level.{}", r.level), &r.level.to_string()))
            } else {
                self.infuser_name(&r.id)
            };
            let color = if incompatible || r.obfuscated() { rgba(0x68, 0x5E, 0x4A, 255) } else { white };
            if r.obfuscated() {
                // EnchantmentNames.getRandomName in the Standard Galactic alphabet, seeded with the screen's
                // seed + the enchantment, cut to 72 % of the row.
                let seed = STATE.with(|s| s.borrow().seed);
                let words = galactic_words(i64::from(seed) + self.infuser_enchant_index(&r.id));
                let max = (ITEM_W * 0.72).floor();
                let w = galactic(batch, atlas, &words, 0.0, 0.0, max, 0, false);
                galactic(batch, atlas, &words, x + ITEM_W / 2.0 - (w / 2.0).floor(), y + 5.0, max, color, true);
            } else {
                let w = HudBatch::text_width(atlas, &name);
                batch.text(atlas, &name, x + ITEM_W / 2.0 - (w / 2.0).floor(), y + 5.0, color, true);
            }
            let hover = |bx: f32| mx >= bx && my >= y && mx < bx + 18.0 && my < y + 18.0;
            let b = r.buttons();
            // The minus and plus (IconButton: inactive, normal, hovered rows of 18).
            for (visible, active, bx, u, increase) in [(b.decr_visible, b.decr_active, x, 220.0, false), (b.incr_visible, b.incr_active, x + ITEM_W - 18.0, 238.0, true)] {
                if !visible {
                    continue;
                }
                let state = if !active { 0.0 } else if hover(bx) { 2.0 } else { 1.0 };
                if active && shift {
                    let (a, b2) = if increase { (bx - 2.0, bx + 4.0) } else { (bx + 2.0, bx - 4.0) };
                    self.infuser_blit(batch, a, y, u, state * 18.0, 18.0, 18.0);
                    self.infuser_blit(batch, b2, y, u, state * 18.0, 18.0, 18.0);
                } else {
                    self.infuser_blit(batch, bx, y, u, state * 18.0, 18.0, 18.0);
                }
                let low = if increase { r.level >= r.allowed } else { r.level.saturating_sub(1) >= r.allowed && r.level > 0 };
                if hover(bx) && low && !r.obfuscated() {
                    let key = if increase { "gui.enchantinginfuser.tooltip.lowPower1" } else { "gui.enchantinginfuser.tooltip.lowPower2" };
                    let mut lines = vec![(power_line(format!("\u{a7}c{}\u{a7}r", view.power)).replacen("%s", &r.required.to_string(), 1), white)];
                    lines.push((self.translate(key, ""), gray));
                    tip = Some(lines);
                    insufficient = true;
                }
            }
            if mx >= x + 18.0 && mx < x + 18.0 + 126.0 && my >= y && my < y + 18.0 {
                if r.obfuscated() {
                    let mut lines = vec![(power_line(format!("\u{a7}c{}\u{a7}r", view.power)).replacen("%s", &r.required.to_string(), 1), white)];
                    lines.push((self.translate("gui.enchantinginfuser.tooltip.unknown_enchantment", ""), gray));
                    tip = Some(lines);
                    insufficient = true;
                } else if incompatible {
                    let names: Vec<String> = r.incompatible.iter().map(|id| self.infuser_name(id)).collect();
                    let text = self.translate("gui.enchantinginfuser.tooltip.incompatible", "%s").replacen("%s", &format!("\u{a7}7{}", names.join(", ")), 1);
                    tip = Some(vec![(text, white)]);
                } else {
                    let min = self.translate("enchantment.level.1", "I");
                    let levels_text = if r.data_max != 1 {
                        format!("{min}-{}", self.translate(&format!("enchantment.level.{}", r.data_max), &r.data_max.to_string()))
                    } else {
                        min
                    };
                    let mut lines = vec![(format!("{} \u{a7}7({levels_text})", self.infuser_name(&r.id)), white)];
                    let key = self.res.stations().enchantments.get(&r.id).map(|x| x.description_key.clone()).unwrap_or_default();
                    for k in [format!("{key}.desc"), format!("{key}.description")] {
                        let d = self.translate(&k, "");
                        if !d.is_empty() && d != k {
                            lines.push((d, gray));
                            break;
                        }
                    }
                    tip = Some(lines);
                }
            }
        }
        // The enchant button and its cost.
        let enchant_y = if cfg.repairing != engine::Repairs::Nothing { ENCHANT_Y } else { ENCHANT_ONLY_Y };
        let can_enchant = view.can_enchant(levels, creative);
        let (bx, by) = (ox + BUTTON_X, oy + enchant_y);
        let over = |x: f32, y: f32| mx >= x && my >= y && mx < x + 18.0 && my < y + 18.0;
        let button_state = |active: bool, x: f32, y: f32| if !active { 0.0 } else if over(x, y) { 2.0 } else { 1.0 };
        self.infuser_blit(batch, bx, by, 126.0, 185.0 + button_state(can_enchant, bx, by) * 18.0, 18.0, 18.0);
        if can_enchant || view.enchant_cost != 0 {
            if view.enchant_cost != 0 {
                let color = if view.enchant_cost < 0 { rgba(255, 255, 85, 255) } else if can_enchant { rgba(85, 255, 85, 255) } else { rgba(255, 85, 85, 255) };
                let text = if view.enchant_cost < 0 { "+".to_owned() } else { view.enchant_cost.to_string() };
                readable(batch, atlas, bx + 1.0, by + 1.0, &text, color);
            }
            if over(bx, by) {
                tip = Some(if can_enchant {
                    let mut lines = self.infuser_result_lines(&view);
                    if view.enchant_cost != 0 {
                        lines.push((String::new(), white));
                        let text = if view.enchant_cost < 0 {
                            self.translate("gui.enchantinginfuser.tooltip.points", "Gain Experience Points")
                        } else if view.enchant_cost == 1 {
                            self.translate("container.enchant.level.one", "1 Enchantment Level")
                        } else {
                            self.translate("container.enchant.level.many", "%s Enchantment Levels").replacen("%s", &view.enchant_cost.to_string(), 1)
                        };
                        lines.push((text, gray));
                    }
                    lines
                } else {
                    vec![(self.translate("container.enchant.level.requirement", "Level Requirement: %s").replacen("%s", &view.enchant_cost.to_string(), 1), rgba(255, 85, 85, 255))]
                });
            }
        }
        // The repair button and its cost.
        if cfg.repairing != engine::Repairs::Nothing {
            let can_repair = view.can_repair(levels, creative);
            let (rx, ry) = (ox + BUTTON_X, oy + REPAIR_Y);
            self.infuser_blit(batch, rx, ry, 144.0, 185.0 + button_state(can_repair, rx, ry) * 18.0, 18.0, 18.0);
            if can_repair || view.repair_cost != 0 {
                if view.repair_cost != 0 {
                    let color = if can_repair { rgba(85, 255, 85, 255) } else { rgba(255, 85, 85, 255) };
                    readable(batch, atlas, rx + 1.0, ry + 1.0, &view.repair_cost.to_string(), color);
                }
                if over(rx, ry) {
                    tip = Some(if can_repair {
                        let mut lines = Vec::new();
                        if let Some(stack) = self.infuser_open_menu().and_then(|p| self.infuser_window_item(p)) {
                            let max = self.res.items().get(&stack.item).map_or(0, |i| i.max_damage);
                            lines.push((self.infuser_stack_name(&stack), white));
                            let change = self.translate("gui.enchantinginfuser.tooltip.change", "%s -> %s")
                                .replacen("%s", &(max.saturating_sub(stack.damage)).to_string(), 1)
                                .replacen("%s", &max.to_string(), 1);
                            lines.push((self.translate("gui.enchantinginfuser.tooltip.durability", "Durability: %s").replacen("%s", &change, 1), rgba(255, 255, 85, 255)));
                            lines.push((String::new(), white));
                        }
                        let text = if view.repair_cost == 1 {
                            self.translate("container.enchant.level.one", "1 Enchantment Level")
                        } else {
                            self.translate("container.enchant.level.many", "%s Enchantment Levels").replacen("%s", &view.repair_cost.to_string(), 1)
                        };
                        lines.push((text, gray));
                        lines
                    } else {
                        vec![(self.translate("container.enchant.level.requirement", "Level Requirement: %s").replacen("%s", &view.repair_cost.to_string(), 1), rgba(255, 85, 85, 255))]
                    });
                }
            }
        }
        // The enchanting power: a bookshelf with the number.
        let (px, py) = (ox + POWER_X, oy + POWER_Y);
        self.draw_item(batch, &ItemStack::new("minecraft:bookshelf", 1), px, py);
        let color = if view.power >= view.max_power { rgba(255, 255, 85, 255) } else if insufficient { rgba(255, 85, 85, 255) } else { white };
        let n = view.power.to_string();
        let w = HudBatch::text_width(atlas, &n);
        batch.text(atlas, &n, px + 19.0 - 2.0 - w, py + 6.0 + 3.0, color, true);
        if mx >= px && my >= py && mx < px + 16.0 && my < py + 16.0 {
            let mut lines = vec![(power_line(view.power.to_string()).replacen("%s", &view.max_power.to_string(), 1), rgba(255, 255, 85, 255))];
            if view.power < view.max_power {
                lines.push((self.translate("gui.enchantinginfuser.tooltip.enchanting_power.hint", ""), gray));
            }
            tip = Some(lines);
        }
        if let Some(lines) = tip {
            self.draw_tooltip_lines(batch, &lines, mx, my);
        }
    }

    /// `BuiltInRegistries.ENCHANTMENT.getId(enchantment)`.
    fn infuser_enchant_index(&self, id: &str) -> i64 {
        self.res.stations().enchantments.list.iter().position(|e| &*e.id == id).map_or(0, |i| i as i64)
    }

    fn infuser_window_item(&self, p: [i32; 3]) -> Option<ItemStack> {
        if self.remote.is_some() {
            // forja's mirror: the block entity's slots at the infuser.
            return self.infuser_slot_item(p);
        }
        self.infuser_slot_item(p)
    }

    fn infuser_stack_name(&self, stack: &ItemStack) -> String {
        stack.custom_name().map(str::to_owned).unwrap_or_else(|| self.pack.names.display_item(&stack.item))
    }

    /// `addEnchantments` under the result's name: new (green), changed (yellow), kept (gray), removed (red).
    fn infuser_result_lines(&self, view: &engine::View) -> Vec<(String, u32)> {
        let Some(stack) = self.infuser_open_menu().and_then(|p| self.infuser_window_item(p)) else {
            return Vec::new();
        };
        let old: Vec<(String, u32)> = (if &*stack.item == "minecraft:enchanted_book" { stack.stored_enchantments() } else { stack.enchantments() })
            .iter()
            .map(|(e, l)| (e.to_string(), *l))
            .collect();
        let enchanted = view.rows.iter().any(|r| r.level > 0);
        let display = if &*stack.item == "minecraft:book" && enchanted { "minecraft:enchanted_book" } else if &*stack.item == "minecraft:enchanted_book" && !enchanted { "minecraft:book" } else { &*stack.item };
        let name = stack.custom_name().map(str::to_owned).unwrap_or_else(|| self.pack.names.display_item(display));
        let rarity = self.res.stations().enchantments.props(display).map(|p| p.rarity.clone()).unwrap_or_default();
        let color = match (rarity.as_str(), enchanted && display != "minecraft:enchanted_book") {
            ("rare" | "epic", true) => rgba(255, 85, 255, 255),
            (_, true) => rgba(85, 255, 255, 255),
            ("uncommon", false) => rgba(255, 255, 85, 255),
            ("rare", false) => rgba(85, 255, 255, 255),
            ("epic", false) => rgba(255, 85, 255, 255),
            _ => u32::MAX,
        };
        let mut lines = vec![(name, color)];
        let level = |l: u32| self.translate(&format!("enchantment.level.{l}"), &l.to_string());
        let plain = |id: &str, l: Option<u32>| {
            let max = self.res.stations().enchantments.get(id).map_or(1, |e| e.max_level);
            match l {
                Some(l) if l != 1 || max != 1 => format!("{} {}", self.infuser_name(id), level(l)),
                _ => self.infuser_name(id),
            }
        };
        let (mut new, mut changed, mut kept, mut removed) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut ids: Vec<String> = view.rows.iter().map(|r| r.id.clone()).collect();
        for (id, _) in &old {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        for id in ids {
            let o = old.iter().find(|(e, _)| *e == id).map_or(-1, |(_, l)| *l as i64);
            let n = view.rows.iter().find(|r| r.id == id).map_or(-1, |r| r.level as i64);
            if n > 0 && o <= 0 {
                new.push((plain(&id, Some(n as u32)), rgba(85, 255, 85, 255)));
            } else if n == 0 && o > 0 {
                removed.push((plain(&id, Some(o as u32)), rgba(255, 85, 85, 255)));
            } else if n > 0 && o > 0 && n != o {
                let change = self.translate("gui.enchantinginfuser.tooltip.change", "%s -> %s").replacen("%s", &level(o as u32), 1).replacen("%s", &level(n as u32), 1);
                changed.push((format!("{} {change}", self.infuser_name(&id)), rgba(255, 255, 85, 255)));
            } else if n > 0 || o > 0 {
                kept.push((plain(&id, Some(if n != 0 { n.max(o) as u32 } else { o as u32 })), rgba(170, 170, 170, 255)));
            }
        }
        lines.extend(new);
        lines.extend(changed);
        lines.extend(kept);
        lines.extend(removed);
        lines
    }

    // ---- the item over the book (InfuserItemRenderer) ----

    /// HEAD `campfire::campfire_geometry`: the item in the slot turning over the open book, growing as the
    /// book opens. Drawn where the world has the slot (the host, a guest's window while the menu is open).
    pub(super) fn infuser_floating_items(&self, camera: glam::DVec3, out: &mut Vec<horizon_client::mesher::Vertex>) {
        let partial = self.clock.fraction() as f32;
        for (pos, book) in &self.be_anims.books {
            if book.open == 0.0 && book.o_open == 0.0 {
                continue;
            }
            if self.infuser_at(*pos).is_none() {
                continue;
            }
            let corner = glam::DVec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
            if (corner + glam::DVec3::splat(0.5)).distance_squared(camera) > 64.0 * 64.0 {
                continue;
            }
            let Some(stack) = self.infuser_slot_item(*pos) else { continue };
            let state = self.resolve_item(&stack, horizon_assets::item_model::DisplayContext::Ground, None, 0);
            if state.is_empty() {
                continue;
            }
            // modelYScale: the GROUND transform's y scale (0.5 a flat item, 0.25 a block), as the height of
            // the posed model.
            let (lo, hi) = state.bounding_box();
            let model_y_scale = (hi.y - lo.y).clamp(0.0, 1.0);
            let t = book.time as f32 + partial;
            let hover = (t / 10.0).sin() * 0.1 + 0.1;
            let openness = book.o_open + (book.open - book.o_open) * partial;
            let scale = openness * 0.8 + 0.2;
            let pose = glam::Mat4::from_translation((corner - camera).as_vec3() + glam::Vec3::new(0.5, 1.0, 0.5))
                * glam::Mat4::from_translation(glam::Vec3::new(0.0, hover + 0.25 * model_y_scale * openness - 0.15 * (1.0 - openness), 0.0))
                * glam::Mat4::from_scale(glam::Vec3::splat(scale))
                * glam::Mat4::from_rotation_y(t / 20.0);
            let light = horizon_client::block_entity_render::vertex_light(self.world.light(pos[0], pos[1], pos[2]));
            self.submit_item_world(&state, pose, light, out);
        }
    }

    // ---- per tick ----

    /// HEAD `storage::tick_container_reach`: the menu kind is registered, the open infuser keeps its slot.
    pub(super) fn infuser_tick(&mut self) {
        infuser_register_menu();
        if self.remote.is_some() {
            return;
        }
        if let Some(key) = self.container
            && self.infuser_at([key.0, key.1, key.2]).is_some()
        {
            self.infuser_ensure_slot([key.0, key.1, key.2]);
        }
    }
}

/// `EnchantmentNames.WORDS`.
const WORDS: &[&str] = &[
    "the", "elder", "scrolls", "klaatu", "berata", "niktu", "xyzzy", "bless", "curse", "light", "darkness",
    "fire", "air", "earth", "water", "hot", "dry", "cold", "wet", "ignite", "snuff", "embiggen", "twist",
    "shorten", "stretch", "fiddle", "destroy", "imbue", "galvanize", "enchant", "free", "limited", "range",
    "of", "towards", "inside", "sphere", "cube", "self", "other", "ball", "mental", "physical", "grow",
    "shrink", "demon", "elemental", "spirit", "animal", "creature", "beast", "humanoid", "undead", "fresh",
    "stale", "phnglui", "mglwnafh", "cthulhu", "rlyeh", "wgahnagl", "fhtagn", "baguette",
];

/// `EnchantmentNames.initSeed(seed)` + `getRandomName`: three or four words.
fn galactic_words(seed: i64) -> String {
    use horizon_core::random::RandomSource;
    let mut r = horizon_core::random::LegacyRandom::new(seed);
    let count = r.next_int_bound(2).unwrap_or(0) + 3;
    (0..count).map(|_| WORDS[r.next_int_bound(WORDS.len() as i32).unwrap_or(0) as usize]).collect::<Vec<_>>().join(" ")
}

/// Text in the Standard Galactic alphabet (`minecraft:alt`, the `font/ascii_sga` sheet), cut to `max`
/// pixels (as the game's enchanting table draws its clues); `draw` false only measures. Returns the width.
#[allow(clippy::too_many_arguments)]
fn galactic(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, text: &str, x: f32, y: f32, max: f32, color: u32, draw: bool) -> f32 {
    let Some(sheet) = atlas.sprites.get("font/ascii_sga").copied() else {
        if draw {
            batch.text(atlas, text, x, y, color, false);
        }
        return HudBatch::text_width(atlas, text);
    };
    let cell = sheet.w / 16;
    let (px, py) = ((sheet.u0 * atlas.width as f32) as u32, (sheet.v0 * atlas.height as f32) as u32);
    let mut cursor = x;
    for ch in text.chars() {
        let advance = if ch == ' ' {
            4.0
        } else {
            let code = ch as u32;
            if !(0x41..=0x5A).contains(&code) && !(0x61..=0x7A).contains(&code) {
                continue;
            }
            let (col, row) = (code % 16, code / 16);
            let (cx, cy) = (px + col * cell, py + row * cell);
            let mut used = 0;
            for dx in (0..cell).rev() {
                if (0..cell).any(|dy| atlas.pixels.get((((cy + dy) * atlas.width + cx + dx) * 4 + 3) as usize).is_some_and(|a| *a != 0)) {
                    used = dx + 1;
                    break;
                }
            }
            let advance = (used as f32 * 8.0 / cell as f32 + 0.5).floor() + 1.0;
            if cursor + advance - x > max {
                break;
            }
            if draw {
                let r = horizon_client::hud::Rect {
                    u0: cx as f32 / atlas.width as f32,
                    v0: cy as f32 / atlas.height as f32,
                    u1: (cx + cell) as f32 / atlas.width as f32,
                    v1: (cy + cell) as f32 / atlas.height as f32,
                    w: cell,
                    h: cell,
                };
                batch.rect_uv(cursor, y, 8.0, 8.0, r, color);
            }
            advance
        };
        if cursor + advance - x > max {
            break;
        }
        cursor += advance;
    }
    cursor - x
}

/// `renderReadableText`: the cost over a button, outlined in black.
fn readable(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, x: f32, y: f32, text: &str, color: u32) {
    let x = x + 19.0 - 2.0 - HudBatch::text_width(atlas, text);
    let y = y + 6.0 + 3.0;
    let black = rgba(0, 0, 0, 255);
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        batch.text(atlas, text, x + dx, y + dy, black, false);
    }
    batch.text(atlas, text, x, y, color, false);
}

// ---- the forja menu (single player's window, LAN friends) ----

fn infuser_menu_claims(game: &mut Game<'_>, _conn: u32, p: [i32; 3]) -> bool {
    if game.infuser_at(p).is_none() {
        return false;
    }
    game.infuser_ensure_slot(p);
    true
}
fn infuser_menu_valid(game: &Game<'_>, _conn: u32, p: [i32; 3]) -> bool {
    game.infuser_at(p).is_some()
}
/// The list as the guest's window draws it (`S2CCompatibleEnchantsMessage` and the data slots).
fn infuser_menu_sections(game: &mut Game<'_>, conn: u32, p: [i32; 3]) -> Vec<Vec<u8>> {
    vec![game.infuser_view(conn, p).unwrap_or_default().encode()]
}
/// A press of the guest, replayed as him (his cursor, window size and Shift); `extra[0]` = his list's
/// first row.
fn infuser_menu_input(game: &mut Game<'_>, _conn: u32, _p: [i32; 3], input: &super::forja::MenuInput) -> bool {
    let super::forja::MenuInput::Mouse { extra, shift, .. } = input else { return false };
    let scroll = usize::from(extra.first().copied().unwrap_or(0));
    game.infuser_press(scroll, *shift)
}
fn infuser_menu_closed(_game: &mut Game<'_>, conn: u32, _p: [i32; 3]) {
    STATE.with(|s| s.borrow_mut().open.remove(&conn));
}
fn infuser_register_menu() {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    super::forja::register_menu(super::forja::MenuKind {
        claims: Some(infuser_menu_claims),
        valid: Some(infuser_menu_valid),
        sections: Some(infuser_menu_sections),
        input: Some(infuser_menu_input),
        closed: Some(infuser_menu_closed),
        ..super::forja::MenuKind::new(MENU_KIND)
    });
}
