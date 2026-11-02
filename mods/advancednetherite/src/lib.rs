// Advanced Netherite for Horizon: the rules, without any game code.
// Written from the 1.20.1 source of the mod (Autovw/AdvancedNetherite, branch 1.20.1,
// version 2.1.3, GPL-3.0) and from the settings files of the pack. No code of the
// original is used; the numbers are quoted with the Java class they come from.
use std::fmt::Write as _;

pub const MOD: &str = "advancednetherite";

/// The four tiers (`ModToolTiers` / `ModArmorTiers`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    Iron,
    Gold,
    Emerald,
    Diamond,
}
pub const TIERS: [Tier; 4] = [Tier::Iron, Tier::Gold, Tier::Emerald, Tier::Diamond];

impl Tier {
    pub fn name(self) -> &'static str {
        match self {
            Tier::Iron => "iron",
            Tier::Gold => "gold",
            Tier::Emerald => "emerald",
            Tier::Diamond => "diamond",
        }
    }
    pub fn from_name(name: &str) -> Option<Tier> {
        TIERS.into_iter().find(|t| t.name() == name)
    }
    /// `ModToolTiers.getUses`.
    pub fn tool_uses(self) -> u32 {
        match self {
            Tier::Iron => 2281,
            Tier::Gold => 2313,
            Tier::Emerald => 2651,
            Tier::Diamond => 3092,
        }
    }
    /// `ModToolTiers.getAttackDamageBonus`.
    pub fn tool_attack_bonus(self) -> f64 {
        match self {
            Tier::Iron | Tier::Gold => 4.0,
            Tier::Emerald | Tier::Diamond => 5.0,
        }
    }
    /// `getEnchantmentValue` of both the tool tier and the armor material.
    pub fn enchantability(self) -> u32 {
        match self {
            Tier::Iron => 15,
            Tier::Gold => 25,
            Tier::Emerald => 20,
            Tier::Diamond => 15,
        }
    }
    /// `ModArmorTiers`'s `durability` (multiplied by the slot's `HEALTH_PER_SLOT`).
    pub fn armor_durability_factor(self) -> u32 {
        match self {
            Tier::Iron => 39,
            Tier::Gold => 41,
            Tier::Emerald => 43,
            Tier::Diamond => 47,
        }
    }
    /// `ModArmorTiers.getToughness`.
    pub fn armor_toughness(self) -> f64 {
        match self {
            Tier::Diamond => 4.0,
            _ => 3.5,
        }
    }
    /// `ModArmorTiers.getKnockbackResistance`.
    pub fn armor_knockback_resistance(self) -> f64 {
        0.1
    }
    /// `ModArmorTiers.getDefenseForType`: `slotProtections` indexed by the slot
    /// (feet, legs, chest, head).
    pub fn armor_defense(self, piece: Piece) -> u32 {
        let table: [u32; 4] = match self {
            Tier::Iron => [4, 6, 8, 4],
            Tier::Gold => [4, 7, 9, 4],
            Tier::Emerald => [4, 7, 9, 4],
            Tier::Diamond => [5, 7, 9, 5],
        };
        table[piece.slot_index()]
    }
    /// `ModArmorTiers.getDurabilityForType`: `HEALTH_PER_SLOT[slot] * durability`.
    pub fn armor_durability(self, piece: Piece) -> u32 {
        const HEALTH_PER_SLOT: [u32; 4] = [13, 15, 16, 11];
        HEALTH_PER_SLOT[piece.slot_index()] * self.armor_durability_factor()
    }
    pub fn ingot(self) -> String {
        format!("{MOD}:netherite_{}_ingot", self.name())
    }
    pub fn block(self) -> String {
        format!("{MOD}:netherite_{}_block", self.name())
    }
    /// The equipment asset of the worn armor (`ModArmorTiers.getName`).
    pub fn armor_asset(self) -> String {
        format!("{MOD}:netherite_{}", self.name())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Piece {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}
pub const PIECES: [Piece; 4] = [Piece::Helmet, Piece::Chestplate, Piece::Leggings, Piece::Boots];
impl Piece {
    pub fn name(self) -> &'static str {
        match self {
            Piece::Helmet => "helmet",
            Piece::Chestplate => "chestplate",
            Piece::Leggings => "leggings",
            Piece::Boots => "boots",
        }
    }
    /// `EquipmentSlot.getIndex` of the armor type: feet 0, legs 1, chest 2, head 3.
    pub fn slot_index(self) -> usize {
        match self {
            Piece::Boots => 0,
            Piece::Leggings => 1,
            Piece::Chestplate => 2,
            Piece::Helmet => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Axe,
    Hoe,
    Pickaxe,
    Shovel,
    Sword,
}
pub const TOOLS: [Tool; 5] = [Tool::Axe, Tool::Hoe, Tool::Pickaxe, Tool::Shovel, Tool::Sword];
impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Axe => "axe",
            Tool::Hoe => "hoe",
            Tool::Pickaxe => "pickaxe",
            Tool::Shovel => "shovel",
            Tool::Sword => "sword",
        }
    }
    /// The attack damage argument of the item's constructor in `ModItems`
    /// (`new AdvancedAxeItem(tier, 5, -3.0f, ...)` and so on).
    pub fn damage_argument(self, tier: Tier) -> f64 {
        use Tier::*;
        match (self, tier) {
            (Tool::Axe, Iron) => 5.0,
            (Tool::Axe, Gold) | (Tool::Axe, Emerald) => 6.0,
            (Tool::Axe, Diamond) => 7.0,
            (Tool::Hoe, Iron) | (Tool::Hoe, Gold) => -4.0,
            (Tool::Hoe, Emerald) | (Tool::Hoe, Diamond) => -5.0,
            (Tool::Pickaxe, _) => 1.0,
            (Tool::Shovel, Iron) | (Tool::Shovel, Gold) => 1.5,
            (Tool::Shovel, Emerald) | (Tool::Shovel, Diamond) => 1.0,
            (Tool::Sword, Iron) => 3.0,
            (Tool::Sword, Gold) | (Tool::Sword, Emerald) => 4.0,
            (Tool::Sword, Diamond) => 5.0,
        }
    }
    /// The attack speed argument (an offset from the base 4.0).
    pub fn speed_argument(self) -> f64 {
        match self {
            Tool::Axe | Tool::Shovel => -3.0,
            Tool::Hoe => 0.0,
            Tool::Pickaxe => -2.8,
            Tool::Sword => -2.4,
        }
    }
    /// `DiggerItem` / `SwordItem`: the attack damage modifier is the argument
    /// plus the tier's bonus.
    pub fn attack_damage_modifier(self, tier: Tier) -> f64 {
        self.damage_argument(tier) + tier.tool_attack_bonus()
    }
    pub fn is_digger(self) -> bool {
        self != Tool::Sword
    }
}

/// The options of the pack: `config/advancednetherite-common.toml` and, for the
/// server options, the world "Prima lume"'s `serverconfig/advancednetherite-server.toml`.
/// Both hold the mod's defaults, so these are the defaults of 2.1.3's `Config`.
#[derive(Clone, Debug)]
pub struct Settings {
    /// Per tier: phantoms, piglins, endermen neutral towards the wearer.
    pub passive: [[bool; 3]; 4],
    pub additional_crop_drops: bool,
    pub additional_ore_drops: bool,
    pub additional_mob_drops: bool,
    /// `breaking_speed_multipliers`: iron, gold, emerald, diamond.
    pub breaking_speed: [f32; 4],
    pub wheat: f32,
    pub carrots: f32,
    pub potatoes: f32,
    pub beetroots: f32,
    pub phantom: f32,
    pub zombified_piglin: f32,
    pub piglin: f32,
    pub enderman: f32,
    pub raw_iron: f32,
    pub raw_gold: f32,
    pub emerald: f32,
    pub diamond: f32,
    pub gold_nugget: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            // [common.armor_perks.*]: phantoms, piglins, endermen.
            passive: [
                [true, false, false], // netherite_iron_armor
                [false, true, false], // netherite_gold_armor
                [false, false, true], // netherite_emerald_armor
                [true, true, true],   // netherite_diamond_armor
            ],
            additional_crop_drops: true,
            additional_ore_drops: true,
            additional_mob_drops: true,
            breaking_speed: [12.0, 20.0, 29.0, 39.0],
            wheat: 0.3,
            carrots: 0.3,
            potatoes: 0.3,
            beetroots: 0.2,
            phantom: 0.5,
            zombified_piglin: 0.5,
            piglin: 0.15,
            enderman: 0.3,
            raw_iron: 0.2,
            raw_gold: 0.3,
            emerald: 0.4,
            diamond: 0.25,
            gold_nugget: 0.6,
        }
    }
}
impl Settings {
    fn tier_index(tier: Tier) -> usize {
        TIERS.iter().position(|t| *t == tier).unwrap_or(0)
    }
    pub fn breaking_speed(&self, tier: Tier) -> f32 {
        self.breaking_speed[Self::tier_index(tier)]
    }
}

/// The three creatures armor can pacify.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pacify {
    Phantoms,
    Piglins,
    Endermen,
}
impl Settings {
    /// `AdvancedArmorItem.pacifiesPhantoms` / `Piglins` / `Endermen`.
    pub fn pacifies(&self, tier: Tier, what: Pacify) -> bool {
        let slot = match what {
            Pacify::Phantoms => 0,
            Pacify::Piglins => 1,
            Pacify::Endermen => 2,
        };
        self.passive[Self::tier_index(tier)][slot]
    }
}

/// The tier and piece of an armor item id (`advancednetherite:netherite_gold_boots`).
pub fn armor_of(item: &str) -> Option<(Tier, Piece)> {
    let rest = item.strip_prefix(MOD)?.strip_prefix(":netherite_")?;
    let (tier, piece) = rest.split_once('_')?;
    let tier = Tier::from_name(tier)?;
    let piece = PIECES.into_iter().find(|p| p.name() == piece)?;
    Some((tier, piece))
}
/// The tier and tool of a tool item id.
pub fn tool_of(item: &str) -> Option<(Tier, Tool)> {
    let rest = item.strip_prefix(MOD)?.strip_prefix(":netherite_")?;
    let (tier, tool) = rest.split_once('_')?;
    let tier = Tier::from_name(tier)?;
    let tool = TOOLS.into_iter().find(|t| t.name() == tool)?;
    Some((tier, tool))
}

/// What the worn pieces pacify: the original asks every worn piece and the
/// effect applies as soon as one of them has the perk
/// (`EventHandler.onEnderManAngerEvent`, `onLivingChangeTargetEvent`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    pub phantoms: bool,
    pub piglins: bool,
    pub endermen: bool,
}
pub fn worn_perks<'a>(s: &Settings, worn: impl IntoIterator<Item = Option<&'a str>>) -> Worn {
    let mut out = Worn::default();
    for item in worn.into_iter().flatten() {
        if let Some((tier, _)) = armor_of(item) {
            out.phantoms |= s.pacifies(tier, Pacify::Phantoms);
            out.piglins |= s.pacifies(tier, Pacify::Piglins);
            out.endermen |= s.pacifies(tier, Pacify::Endermen);
        }
    }
    out
}

// ---------------------------------------------------------------- the items

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ingot(Tier),
    Armor(Tier, Piece),
    Tool(Tier, Tool),
}

/// One item the mod adds (the 4 blocks are declared as blocks with their item).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// Without the namespace.
    pub path: String,
    pub kind: Kind,
    /// The vanilla item whose components it copies.
    pub template: &'static str,
}
impl Spec {
    pub fn id(&self) -> String {
        format!("{MOD}:{}", self.path)
    }
}

/// Every item with its own declaration, in `ModItems` order.
pub fn items() -> Vec<Spec> {
    let mut out = Vec::new();
    for tier in TIERS {
        out.push(Spec {
            path: format!("netherite_{}_ingot", tier.name()),
            kind: Kind::Ingot(tier),
            // A plain item that burns like netherite: netherite_ingot would also
            // be a trim material.
            template: "minecraft:netherite_scrap",
        });
    }
    for tier in TIERS {
        for piece in PIECES {
            out.push(Spec {
                path: format!("netherite_{}_{}", tier.name(), piece.name()),
                kind: Kind::Armor(tier, piece),
                template: match piece {
                    Piece::Helmet => "minecraft:netherite_helmet",
                    Piece::Chestplate => "minecraft:netherite_chestplate",
                    Piece::Leggings => "minecraft:netherite_leggings",
                    Piece::Boots => "minecraft:netherite_boots",
                },
            });
        }
    }
    for tool in TOOLS {
        for tier in TIERS {
            out.push(Spec {
                path: format!("netherite_{}_{}", tier.name(), tool.name()),
                kind: Kind::Tool(tier, tool),
                template: match tool {
                    Tool::Axe => "minecraft:netherite_axe",
                    Tool::Hoe => "minecraft:netherite_hoe",
                    Tool::Pickaxe => "minecraft:netherite_pickaxe",
                    Tool::Shovel => "minecraft:netherite_shovel",
                    Tool::Sword => "minecraft:netherite_sword",
                },
            });
        }
    }
    out
}
/// The four blocks: `Block(BlockBehaviour.Properties.copy(Blocks.NETHERITE_BLOCK))`.
pub fn blocks() -> Vec<(String, &'static str)> {
    TIERS
        .into_iter()
        .map(|t| (format!("netherite_{}_block", t.name()), "minecraft:netherite_block"))
        .collect()
}

// ------------------------------------------------------ components of an item

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Int(i64),
    Float(f64),
    Str(String),
}

/// One change to the template's components (`items.json`'s entry).
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Sets `path[..]` (objects made on the way) to a value.
    Set(Vec<&'static str>, Val),
    /// `minecraft:attribute_modifiers`: the amount of the modifier of this attribute.
    Attribute(&'static str, f64),
    /// `minecraft:tool`: the speed of the rules that are `correct_for_drops`.
    ToolSpeed(f64),
}

/// The changes that make the template item this item. `s` supplies the
/// breaking speed multipliers.
pub fn ops(spec: &Spec, s: &Settings) -> Vec<Op> {
    let mut out = Vec::new();
    match spec.kind {
        Kind::Ingot(_) => {}
        Kind::Armor(tier, piece) => {
            out.push(Op::Set(
                vec!["minecraft:max_damage"],
                Val::Int(i64::from(tier.armor_durability(piece))),
            ));
            out.push(Op::Set(
                vec!["minecraft:enchantable", "value"],
                Val::Int(i64::from(tier.enchantability())),
            ));
            out.push(Op::Set(
                vec!["minecraft:repairable", "items"],
                Val::Str(tier.ingot()),
            ));
            out.push(Op::Set(
                vec!["minecraft:equippable", "asset_id"],
                Val::Str(tier.armor_asset()),
            ));
            out.push(Op::Attribute(
                "minecraft:armor",
                f64::from(tier.armor_defense(piece)),
            ));
            out.push(Op::Attribute("minecraft:armor_toughness", tier.armor_toughness()));
            out.push(Op::Attribute(
                "minecraft:knockback_resistance",
                tier.armor_knockback_resistance(),
            ));
        }
        Kind::Tool(tier, tool) => {
            out.push(Op::Set(
                vec!["minecraft:max_damage"],
                Val::Int(i64::from(tier.tool_uses())),
            ));
            out.push(Op::Set(
                vec!["minecraft:enchantable", "value"],
                Val::Int(i64::from(tier.enchantability())),
            ));
            out.push(Op::Set(
                vec!["minecraft:repairable", "items"],
                Val::Str(tier.ingot()),
            ));
            out.push(Op::Attribute(
                "minecraft:attack_damage",
                tool.attack_damage_modifier(tier),
            ));
            out.push(Op::Attribute("minecraft:attack_speed", tool.speed_argument()));
            if tool.is_digger() {
                // AdvancedUtil.getDestroySpeed: the tier's speed is 1.0 and the
                // multiplier applies where the tool is the right one for the block.
                out.push(Op::ToolSpeed(f64::from(s.breaking_speed(tier))));
            }
        }
    }
    out
}

/// The patch of the item `id` (`advancednetherite:...`), if it is one of ours.
pub fn patch_for(id: &str, s: &Settings) -> Option<Vec<Op>> {
    let path = id.strip_prefix(MOD)?.strip_prefix(':')?;
    items().into_iter().find(|i| i.path == path).map(|i| ops(&i, s))
}

/// A JSON-like tree the patches are applied to (the game's own tree type).
pub trait Node: Sized {
    fn object() -> Self;
    fn int(v: i64) -> Self;
    fn float(v: f64) -> Self;
    fn string(v: &str) -> Self;
    /// A member of an object.
    fn member(&mut self, key: &str) -> Option<&mut Self>;
    fn put(&mut self, key: &str, value: Self);
    fn elements(&mut self) -> Option<&mut Vec<Self>>;
    fn text(&self) -> Option<&str>;
    fn flag(&self) -> Option<bool>;
}

fn value<N: Node>(v: &Val) -> N {
    match v {
        Val::Int(i) => N::int(*i),
        Val::Float(f) => N::float(*f),
        Val::Str(s) => N::string(s),
    }
}

/// Sets the member at `path`, making the objects on the way.
fn set_path<N: Node>(node: &mut N, path: &[&str], v: N) {
    match path {
        [] => {}
        [last] => node.put(last, v),
        [first, rest @ ..] => {
            if node.member(first).is_none() {
                node.put(first, N::object());
            }
            if let Some(child) = node.member(first) {
                set_path(child, rest, v);
            }
        }
    }
}

/// Applies the changes to one item's components.
pub fn apply<N: Node>(components: &mut N, ops: &[Op]) {
    for op in ops {
        match op {
            Op::Set(path, v) => set_path(components, path, value(v)),
            Op::Attribute(attribute, amount) => {
                let Some(list) = components
                    .member("minecraft:attribute_modifiers")
                    .and_then(N::elements)
                else {
                    continue;
                };
                for modifier in list.iter_mut() {
                    if modifier.member("type").and_then(|t| t.text().map(str::to_owned))
                        == Some((*attribute).to_owned())
                    {
                        modifier.put("amount", N::float(*amount));
                    }
                }
            }
            Op::ToolSpeed(speed) => {
                let Some(rules) = components
                    .member("minecraft:tool")
                    .and_then(|t| t.member("rules"))
                    .and_then(N::elements)
                else {
                    continue;
                };
                for rule in rules.iter_mut() {
                    let correct = rule
                        .member("correct_for_drops")
                        .and_then(|c| c.flag())
                        .unwrap_or(false);
                    if correct && rule.member("speed").is_some() {
                        rule.put("speed", N::float(*speed));
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------- additional drops

/// Random draws the way the loot context gives them.
pub trait Rng {
    fn next_float(&mut self) -> f32;
    fn next_int_bound(&mut self, bound: i32) -> i32;
}
/// `RandomSource.nextIntBetweenInclusive`.
fn between(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    rng.next_int_bound(max - min + 1) + min
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Extra {
    pub item: &'static str,
    pub count: i32,
}

/// `CropDropsLootModifier`: a hoe of the mod, a grown crop.
/// `mature` is `CropBlock.isMaxAge(state)`.
pub fn crop_drops(
    s: &Settings,
    tool: &str,
    block: &str,
    mature: bool,
    rng: &mut impl Rng,
) -> Vec<Extra> {
    let mut out = Vec::new();
    // global_loot_modifiers: match_tool with the four hoes.
    if !matches!(tool_of(tool), Some((_, Tool::Hoe))) || !s.additional_crop_drops || !mature {
        return out;
    }
    let mut add = |item, count| {
        if count > 0 {
            out.push(Extra { item, count });
        }
    };
    if block == "minecraft:wheat" && rng.next_float() <= s.wheat {
        add("minecraft:wheat", between(rng, 0, 2));
    }
    if block == "minecraft:carrots" && rng.next_float() <= s.carrots {
        add("minecraft:carrot", between(rng, 0, 2));
    }
    if block == "minecraft:potatoes" && rng.next_float() <= s.potatoes {
        add("minecraft:potato", between(rng, 0, 1));
    }
    if block == "minecraft:beetroots" && rng.next_float() <= s.beetroots {
        add("minecraft:beetroot", between(rng, 1, 2));
    }
    out
}

/// `OreDropsLootModifier`: the matching pickaxe of the mod on its ore, no silk touch.
pub fn ore_drops(
    s: &Settings,
    tool: &str,
    block: &str,
    silk_touch: bool,
    rng: &mut impl Rng,
) -> Vec<Extra> {
    let mut out = Vec::new();
    if !s.additional_ore_drops || silk_touch {
        return out;
    }
    let tier = match tool_of(tool) {
        Some((tier, Tool::Pickaxe)) => tier,
        _ => return out,
    };
    let mut add = |item, count| {
        if count > 0 {
            out.push(Extra { item, count });
        }
    };
    if matches!(block, "minecraft:iron_ore" | "minecraft:deepslate_iron_ore")
        && tier == Tier::Iron
        && rng.next_float() <= s.raw_iron
    {
        add("minecraft:raw_iron", between(rng, 1, 2));
    }
    if matches!(block, "minecraft:gold_ore" | "minecraft:deepslate_gold_ore")
        && tier == Tier::Gold
        && rng.next_float() <= s.raw_gold
    {
        add("minecraft:raw_gold", between(rng, 1, 1));
    }
    if matches!(block, "minecraft:emerald_ore" | "minecraft:deepslate_emerald_ore")
        && tier == Tier::Emerald
        && rng.next_float() <= s.emerald
    {
        add("minecraft:emerald", between(rng, 1, 1));
    }
    if matches!(block, "minecraft:diamond_ore" | "minecraft:deepslate_diamond_ore")
        && tier == Tier::Diamond
        && rng.next_float() <= s.diamond
    {
        add("minecraft:diamond", between(rng, 1, 1));
    }
    if block == "minecraft:nether_gold_ore" && tier == Tier::Gold && rng.next_float() <= s.gold_nugget
    {
        add("minecraft:gold_nugget", between(rng, 1, 3));
    }
    out
}

/// `MobDropsLootModifier`: a player kills a creature holding one of the swords.
pub fn mob_drops(s: &Settings, weapon: &str, victim: &str, rng: &mut impl Rng) -> Vec<Extra> {
    let mut out = Vec::new();
    if !s.additional_mob_drops {
        return out;
    }
    let tier = match tool_of(weapon) {
        Some((tier, Tool::Sword)) => tier,
        _ => return out,
    };
    let gold_or_diamond = matches!(tier, Tier::Gold | Tier::Diamond);
    let emerald_or_diamond = matches!(tier, Tier::Emerald | Tier::Diamond);
    let mut add = |item, count| {
        if count > 0 {
            out.push(Extra { item, count });
        }
    };
    if tier == Tier::Iron && victim == "minecraft:phantom" && rng.next_float() <= s.phantom {
        add("minecraft:phantom_membrane", between(rng, 0, 2));
    }
    if gold_or_diamond
        && victim == "minecraft:zombified_piglin"
        && rng.next_float() <= s.zombified_piglin
    {
        add("minecraft:gold_nugget", between(rng, 0, 3));
    }
    if gold_or_diamond && victim == "minecraft:piglin" && rng.next_float() <= s.piglin {
        add("minecraft:gold_ingot", between(rng, 1, 1));
    }
    if emerald_or_diamond && victim == "minecraft:enderman" && rng.next_float() <= s.enderman {
        add("minecraft:ender_pearl", between(rng, 0, 1));
    }
    out
}

/// `CropBlock.isMaxAge` for the four crops the modifier knows: the state's `age`
/// against the crop's last age.
pub fn crop_is_mature(block: &str, age: Option<i32>) -> bool {
    let max = match block {
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => 7,
        "minecraft:beetroots" => 3,
        _ => return false,
    };
    age.is_some_and(|a| a >= max)
}

/// A summary of the perks for the tests and the README (one line per item kind).
pub fn describe(s: &Settings) -> String {
    let mut out = String::new();
    for tier in TIERS {
        let _ = writeln!(
            out,
            "{}: phantoms {} piglins {} endermen {}, speed x{}",
            tier.name(),
            s.pacifies(tier, Pacify::Phantoms),
            s.pacifies(tier, Pacify::Piglins),
            s.pacifies(tier, Pacify::Endermen),
            s.breaking_speed(tier)
        );
    }
    out
}
