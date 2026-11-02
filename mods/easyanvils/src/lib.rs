// Easy Anvils for Horizon: the rules, without any game code.
// Written from Easy Anvils 8.0.2 for Forge 1.20.1 (Fuzs, MPL-2.0, the jar the pack plays; sources
// github.com/Fuzss/easyanvils branch 1.20.1) and the pack's config/easyanvils-server.toml and
// easyanvils-client.toml. The Java class each rule follows is named next to it:
// ModAnvilMenu (costs), ServerConfig, ItemInteractionHandler (wear, iron block repair),
// AnvilBlockEntity (the two slots of an anvil), AnvilRenderer (where the items lie).
use std::collections::BTreeMap;

pub type Pos = [i32; 3];

/// `ServerConfig` with the pack's `easyanvils-server.toml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub anvil_break_chance: f64,
    pub risk_free_anvil_renaming: bool,
    pub edit_name_tags_no_anvil: bool,
    pub renaming_supports_formatting: bool,
    pub anvil_repairing: bool,
    pub rare_multiplier: i32,
    pub repair_with_other_item_cost: i32,
    pub common_multiplier: i32,
    pub very_rare_multiplier: i32,
    pub uncommon_multiplier: i32,
    pub halved_book_costs: bool,
    pub repair_with_material_unit_cost: i32,
    /// -1: no limit.
    pub too_expensive_limit: i32,
    pub free_renames: FreeRenames,
    pub rename_and_repair_costs: RenameAndRepairCost,
    pub prior_work_penalty: PriorWorkPenalty,
    pub penalty_free_renames_and_repairs: bool,
    pub prior_work_penalty_constant: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeRenames {
    Off,
    AllItems,
    NameTagsOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameAndRepairCost {
    Vanilla,
    Fixed,
    Limited,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriorWorkPenalty {
    Disabled,
    Vanilla,
    Fixed,
}
impl Config {
    /// the pack's values.
    pub const MARIAN: Config = Config {
        anvil_break_chance: 0.05,
        risk_free_anvil_renaming: true,
        edit_name_tags_no_anvil: true,
        renaming_supports_formatting: true,
        anvil_repairing: true,
        rare_multiplier: 4,
        repair_with_other_item_cost: 2,
        common_multiplier: 1,
        very_rare_multiplier: 8,
        uncommon_multiplier: 2,
        halved_book_costs: true,
        repair_with_material_unit_cost: 1,
        too_expensive_limit: -1,
        free_renames: FreeRenames::AllItems,
        rename_and_repair_costs: RenameAndRepairCost::Fixed,
        prior_work_penalty: PriorWorkPenalty::Fixed,
        penalty_free_renames_and_repairs: true,
        prior_work_penalty_constant: 4,
    };
}
/// `renderAnvilContents` of `easyanvils-client.toml`.
pub const RENDER_ANVIL_CONTENTS: bool = true;

/// The part of an item stack the anvil reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    pub damage: i32,
    /// 0 for an item that cannot be damaged (`isDamageableItem`).
    pub max_damage: i32,
    /// `getBaseRepairCost`.
    pub repair_cost: i32,
    pub custom_name: Option<String>,
    /// `EnchantmentHelper.getEnchantments`: the stored ones for a book.
    pub enchants: Vec<(String, i32)>,
}
impl Stack {
    pub fn is_book(&self) -> bool {
        self.item == "minecraft:enchanted_book"
    }
}

/// What the game knows about enchantments and repair materials.
pub trait Rules {
    /// `Enchantment.canEnchant(stack)`.
    fn can_enchant(&self, id: &str, item: &str) -> bool;
    /// `Enchantment.isCompatibleWith`.
    fn compatible(&self, a: &str, b: &str) -> bool;
    /// 0 for an enchantment the game does not know (skipped).
    fn max_level(&self, id: &str) -> i32;
    /// The multiplier class: 1, 2, 4 or 8 is `Rarity` common, uncommon, rare, very rare
    /// (1.21's `anvil_cost` is the same number).
    fn anvil_cost(&self, id: &str) -> i32;
    /// `Item.isValidRepairItem(stack, material)`.
    fn valid_repair(&self, stack: &Stack, material: &Stack) -> bool;
}

/// What `createResult` leaves in the menu.
#[derive(Clone, Debug, PartialEq)]
pub struct Out {
    pub result: Option<Stack>,
    /// The `cost` data slot (-1: too expensive for a stack of several items).
    pub cost: i32,
    /// `repairItemCountCost`.
    pub repair_item_count_cost: u32,
    /// Horizon's `onlyRenaming` (the right slot stays); 1.20.1 never keeps it.
    pub only_renaming: bool,
}

/// `ModAnvilMenu.repairCostToRepairs`.
pub fn repair_cost_to_repairs(repair_cost: i32) -> i32 {
    let mut cost = repair_cost.saturating_add(1);
    let mut repairs = 0;
    while cost >= 2 {
        cost /= 2;
        repairs += 1;
    }
    repairs
}

/// `AnvilMenu.calculateIncreasedRepairCost`.
pub fn increased_repair_cost(cost: i32) -> i32 {
    cost.saturating_mul(2).saturating_add(1)
}

/// `PriorWorkPenalty.operator`.
pub fn prior_work(cfg: &Config, repair_cost: i32) -> i32 {
    match cfg.prior_work_penalty {
        PriorWorkPenalty::Disabled => 0,
        PriorWorkPenalty::Vanilla => repair_cost,
        PriorWorkPenalty::Fixed => {
            repair_cost_to_repairs(repair_cost).saturating_mul(cfg.prior_work_penalty_constant)
        }
    }
}

/// `ComponentDecomposer.getStringLength`: the length of a name without its `§x` formatting codes.
pub fn formatted_length(name: &str) -> usize {
    let mut n = 0;
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' && chars.peek().is_some_and(|c| is_code(*c)) {
            chars.next();
            continue;
        }
        n += 1;
    }
    n
}
fn is_code(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r')
}

/// `FormattedStringDecomposer.filterText`: every character but control ones (`§` stays).
pub fn filter_text(input: &str) -> String {
    input.chars().filter(|&c| c >= ' ' && c != '\u{7f}').collect()
}

fn multiplier(cfg: &Config, anvil_cost: i32, book: bool) -> i32 {
    let m = match anvil_cost {
        1 => cfg.common_multiplier,
        2 => cfg.uncommon_multiplier,
        4 => cfg.rare_multiplier,
        _ => cfg.very_rare_multiplier,
    };
    if book && cfg.halved_book_costs { (m / 2).max(1) } else { m }
}

fn free_rename(cfg: &Config, left: &Stack) -> bool {
    match cfg.free_renames {
        FreeRenames::Off => false,
        FreeRenames::AllItems => true,
        FreeRenames::NameTagsOnly => left.item == "minecraft:name_tag",
    }
}

/// `ModAnvilMenu.createResult(left, right, itemName)`. `name` is the rename box text (None: the
/// box was not touched); `hover_name` is the left item's name as the box would show it.
pub fn anvil_result(
    rules: &dyn Rules,
    cfg: &Config,
    left: Option<&Stack>,
    right: Option<&Stack>,
    name: Option<&str>,
    hover_name: &str,
    creative: bool,
) -> Out {
    let none = |cost: i32| Out { result: None, cost, repair_item_count_cost: 0, only_renaming: false };
    let Some(left) = left else {
        return none(0);
    };
    let mut output = left.clone();
    let mut map: Vec<(String, i32)> = output.enchants.clone();
    let mut base_repair_cost = left.repair_cost + right.map_or(0, |r| r.repair_cost);
    base_repair_cost = prior_work(cfg, base_repair_cost);
    let mut repair_item_count = 0u32;
    let mut repair_cost = 0;
    let mut enchant_cost = 0;
    let mut rename_cost = 0;
    if let Some(right) = right {
        let is_book = right.is_book() && !right.enchants.is_empty();
        if output.max_damage > 0 && rules.valid_repair(left, right) {
            let mut l2 = output.damage.min(output.max_damage / 4);
            if l2 <= 0 {
                return none(0);
            }
            let mut materials = 0u32;
            while l2 > 0 && materials < right.count {
                output.damage -= l2;
                repair_cost += cfg.repair_with_material_unit_cost;
                l2 = output.damage.min(output.max_damage / 4);
                materials += 1;
            }
            repair_item_count = materials;
        } else {
            if !is_book && (output.item != right.item || output.max_damage <= 0) {
                return none(0);
            }
            if output.max_damage > 0 && !is_book {
                let l = left.max_damage - left.damage;
                let i1 = right.max_damage - right.damage;
                let j1 = i1 + output.max_damage * 12 / 100;
                let k1 = l + j1;
                let l1 = (output.max_damage - k1).max(0);
                if l1 < output.damage {
                    output.damage = l1;
                    repair_cost += cfg.repair_with_other_item_cost;
                }
            }
            let mut flag2 = false;
            let mut flag3 = false;
            for (id, right_level) in &right.enchants {
                if rules.max_level(id) <= 0 {
                    continue;
                }
                let other = map.iter().find(|(e, _)| e == id).map_or(0, |(_, l)| *l);
                let mut level = if other == *right_level { right_level + 1 } else { (*right_level).max(other) };
                let mut compatible = rules.can_enchant(id, &left.item);
                if creative || left.is_book() {
                    compatible = true;
                }
                for (e, _) in &map {
                    if e != id && !rules.compatible(id, e) {
                        compatible = false;
                        enchant_cost += 1;
                    }
                }
                if !compatible {
                    // an item with only incompatible enchantments still repairs
                    if repair_cost > 0 {
                        continue;
                    }
                    flag3 = true;
                } else {
                    flag2 = true;
                    let max = rules.max_level(id);
                    if level > max {
                        level = max;
                    }
                    // a level above the maximum is not lowered to it
                    let max_level = other.max(*right_level).max(level);
                    if max_level != level {
                        level = max_level;
                    }
                    let mult = multiplier(cfg, rules.anvil_cost(id), is_book);
                    let old = map.iter().position(|(e, _)| e == id);
                    match old {
                        Some(i) => {
                            let was = map[i].1;
                            map[i].1 = level;
                            if was != level {
                                enchant_cost += mult * level;
                            }
                        }
                        None => {
                            map.push((id.clone(), level));
                            enchant_cost += mult * level;
                        }
                    }
                    if left.count > 1 && !creative {
                        return none(-1);
                    }
                }
            }
            if flag3 && !flag2 {
                return none(0);
            }
        }
    }
    let mut has_renamed = false;
    let (named, shown) = match name {
        Some(n) => (formatted_length(n) > 0, n),
        None => (false, ""),
    };
    if !named {
        if left.custom_name.is_some() {
            rename_cost = if free_rename(cfg, left) { 0 } else { 1 };
            has_renamed = true;
            output.custom_name = None;
        }
    } else if shown != hover_name {
        rename_cost = if free_rename(cfg, left) { 0 } else { 1 };
        has_renamed = true;
        output.custom_name = Some(shown.to_owned());
    }
    let mut result = Some(output);
    let all = enchant_cost + repair_cost + rename_cost;
    let mut cost;
    if all == 0 {
        cost = 0;
        // free renames still pass
        if !has_renamed {
            result = None;
        }
    } else if enchant_cost == 0 && cfg.rename_and_repair_costs == RenameAndRepairCost::Fixed {
        cost = all;
    } else {
        cost = base_repair_cost + all;
    }
    let no_limit = cfg.too_expensive_limit == -1;
    let max_cost = if no_limit { 40 } else { cfg.too_expensive_limit };
    if cost >= max_cost {
        if enchant_cost == 0 && cfg.rename_and_repair_costs == RenameAndRepairCost::Limited {
            cost = max_cost - 1;
        } else if !no_limit && !creative {
            result = None;
        }
    }
    if let Some(out) = result.as_mut() {
        let mut out_cost = out.repair_cost;
        if let Some(r) = right
            && out_cost < r.repair_cost
        {
            out_cost = r.repair_cost;
        }
        if all > 0 && (enchant_cost > 0 || !cfg.penalty_free_renames_and_repairs) {
            out_cost = increased_repair_cost(out_cost);
        }
        // no tag when there is no repair cost
        if out_cost > 0 {
            out.repair_cost = out_cost;
        }
        out.enchants = map;
    }
    Out {
        result,
        cost,
        repair_item_count_cost: repair_item_count,
        // 1.20.1 always uses up the right slot (nothing to keep): no `onlyRenaming`.
        only_renaming: false,
    }
}

/// `AnvilMenu.mayPickup` of Easy Anvils: `cost >= 0` (free renames) instead of `> 0`.
pub fn may_pickup(creative: bool, levels: i32, cost: i32) -> bool {
    (creative || levels >= cost) && cost >= 0
}

// ---- wear and repair of the block (`AnvilBlock.damage`, `ItemInteractionHandler`) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wear {
    /// Nothing happens (level event 1030).
    None,
    /// The anvil becomes this block (level event 1030).
    Becomes(&'static str),
    /// The anvil breaks (level event 1029).
    Break,
}

/// `ItemInteractionHandler.onAnvilRepair` and `AnvilBlock.damage`: the chance is 0 for a rename
/// alone (`risk_free_anvil_renaming`, the right slot empty), else `anvil_break_chance`;
/// `roll` is `random.nextFloat()`.
pub fn wear(cfg: &Config, block: &str, right_empty: bool, creative: bool, roll: f32) -> Wear {
    let chance: f32 =
        if cfg.risk_free_anvil_renaming && right_empty { 0.0 } else { cfg.anvil_break_chance as f32 };
    if creative || !is_anvil(block) || !(roll < chance) {
        return Wear::None;
    }
    match block {
        "minecraft:anvil" => Wear::Becomes("minecraft:chipped_anvil"),
        "minecraft:chipped_anvil" => Wear::Becomes("minecraft:damaged_anvil"),
        _ => Wear::Break,
    }
}

pub fn is_anvil(block: &str) -> bool {
    matches!(block, "minecraft:anvil" | "minecraft:chipped_anvil" | "minecraft:damaged_anvil")
}

/// `ItemInteractionHandler.getRepairedState`: one stage up, None for a whole anvil.
pub fn repaired(block: &str) -> Option<&'static str> {
    match block {
        "minecraft:damaged_anvil" => Some("minecraft:chipped_anvil"),
        "minecraft:chipped_anvil" => Some("minecraft:anvil"),
        _ => None,
    }
}

// ---- the contents of an anvil (`AnvilBlockEntity`) ----

pub const SLOTS: usize = 2;

/// The two input slots of each anvil.
#[derive(Clone, Debug)]
pub struct Store<S> {
    pub anvils: BTreeMap<Pos, Vec<Option<S>>>,
}
impl<S> Default for Store<S> {
    fn default() -> Self {
        Self { anvils: BTreeMap::new() }
    }
}
impl<S: Clone + PartialEq> Store<S> {
    /// The menu opens: its two slots start with the anvil's items.
    pub fn open(&mut self, pos: Pos) -> Vec<Option<S>> {
        self.anvils.entry(pos).or_insert_with(|| vec![None; SLOTS]).clone()
    }
    /// The open menu's inputs are copied to the anvil. True when they changed.
    pub fn mirror(&mut self, pos: Pos, inputs: &[Option<S>]) -> bool {
        let mut v = inputs.to_vec();
        v.resize(SLOTS, None);
        let slot = self.anvils.entry(pos).or_insert_with(|| vec![None; SLOTS]);
        if *slot == v {
            return false;
        }
        *slot = v;
        true
    }
    /// `AnvilBlock.onRemove`: the anvils whose block is gone (`is_anvil` None: unloaded).
    pub fn take_gone(&mut self, is_anvil: impl Fn(Pos) -> Option<bool>) -> Vec<(Pos, Vec<Option<S>>)> {
        let gone: Vec<Pos> =
            self.anvils.keys().copied().filter(|p| is_anvil(*p) == Some(false)).collect();
        gone.into_iter().filter_map(|p| self.anvils.remove(&p).map(|s| (p, s))).collect()
    }
}

/// Whether a player whose anvil menu at `pos` opens takes the anvil's two slots. The original's menu is built
/// over the one block entity (everyone at an anvil shares it); Horizon gives each player slots of his own (the
/// in-frame host's station, a server guest's menu), so an anvil's contents go to one player at a time: nobody
/// else may hold it (`holders`: who holds which anvil of this level) and the player's slots must be empty. A
/// player who does not take it uses a plain anvil: his items go back to him when he closes it.
pub fn may_bind<W: PartialEq>(holders: &[(Pos, W)], pos: Pos, who: &W, slots_empty: bool) -> bool {
    slots_empty && !holders.iter().any(|(p, w)| *p == pos && w != who)
}

/// The file text: `A x y z`, then `S slot stack` lines.
pub fn save<S>(store: &Store<S>, encode: impl Fn(&S) -> String) -> String {
    let mut out = String::new();
    for (p, slots) in &store.anvils {
        if slots.iter().all(Option::is_none) {
            continue;
        }
        out.push_str(&format!("A\t{}\t{}\t{}\n", p[0], p[1], p[2]));
        for (i, s) in slots.iter().enumerate() {
            if let Some(s) = s {
                out.push_str(&format!("S\t{i}\t{}\n", encode(s)));
            }
        }
    }
    out
}

/// The store of a file text; lines that do not parse are skipped.
pub fn load<S: Clone + PartialEq>(text: &str, decode: impl Fn(&str) -> Option<S>) -> Store<S> {
    let mut store = Store::default();
    let mut at: Option<Pos> = None;
    for line in text.lines() {
        let mut f = line.splitn(3, '\t');
        match (f.next(), f.next(), f.next()) {
            (Some("A"), Some(x), Some(rest)) => {
                let mut r = rest.split('\t');
                at = match (x.parse(), r.next().and_then(|v| v.parse().ok()), r.next().and_then(|v| v.parse().ok())) {
                    (Ok(x), Some(y), Some(z)) => Some([x, y, z]),
                    _ => None,
                };
                if let Some(p) = at {
                    store.anvils.entry(p).or_insert_with(|| vec![None; SLOTS]);
                }
            }
            (Some("S"), Some(i), Some(s)) => {
                if let (Some(p), Ok(i)) = (at, i.parse::<usize>())
                    && i < SLOTS
                    && let Some(stack) = decode(s)
                {
                    store.anvils.entry(p).or_insert_with(|| vec![None; SLOTS])[i] = Some(stack);
                }
            }
            _ => {}
        }
    }
    store
}

// ---- where the items lie on the anvil (`AnvilRenderer.renderFlatItem`) ----

/// The pose of one slot's item: the steps of the original in order, from the block's corner. The
/// facing is the anvil's `facing` (`north`, `south`, `east`, `west`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlatPose {
    /// `translate(0, 1.0375, 0)` then `rotate X by -90 degrees`.
    pub lift: [f32; 3],
    /// Then, in order: rotation about z in degrees (0 for none) and a translation.
    pub spin_z: f32,
    pub shift: [f32; 3],
    pub scale: f32,
}

/// `renderFlatItem`'s transforms; None for a facing that is not horizontal. Axis `X` is east/west,
/// `Z` north/south; `step` is the facing's axis direction (+1 for south, east).
pub fn flat_pose(facing: &str, index: usize) -> Option<FlatPose> {
    let (axis_x, step) = match facing {
        "east" => (true, 1),
        "west" => (true, -1),
        "south" => (false, 1),
        "north" => (false, -1),
        _ => return None,
    };
    let mirrored = (if step == 1 { 1 } else { 0 }) != index % 2;
    let (spin_z, shift) = if axis_x {
        if mirrored { (0.0, [0.25, -0.5, 0.0]) } else { (180.0, [-0.75, 0.5, 0.0]) }
    } else if mirrored {
        (-90.0, [0.25, 0.5, 0.0])
    } else {
        (90.0, [-0.75, -0.5, 0.0])
    };
    Some(FlatPose { lift: [0.0, 1.0375, 0.0], spin_z, shift, scale: 0.375 })
}
