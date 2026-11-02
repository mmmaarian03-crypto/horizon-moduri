// Tests of the Advanced Netherite rules. The numbers are the ones of the 1.20.1 source
// (ModToolTiers, ModArmorTiers, ModItems, the three loot modifiers and the pack's
// advancednetherite-common.toml).
use horizon_advancednetherite::*;

// ------------------------------------------------------------ a tiny JSON tree

#[derive(Clone, Debug, PartialEq)]
enum T {
    Obj(Vec<(String, T)>),
    Arr(Vec<T>),
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}
impl Node for T {
    fn object() -> Self {
        T::Obj(Vec::new())
    }
    fn int(v: i64) -> Self {
        T::Int(v)
    }
    fn float(v: f64) -> Self {
        T::Float(v)
    }
    fn string(v: &str) -> Self {
        T::Str(v.to_owned())
    }
    fn member(&mut self, key: &str) -> Option<&mut Self> {
        match self {
            T::Obj(o) => o.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn put(&mut self, key: &str, value: Self) {
        if let T::Obj(o) = self {
            match o.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => *v = value,
                None => o.push((key.to_owned(), value)),
            }
        }
    }
    fn elements(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            T::Arr(a) => Some(a),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        match self {
            T::Str(s) => Some(s),
            _ => None,
        }
    }
    fn flag(&self) -> Option<bool> {
        match self {
            T::Bool(b) => Some(*b),
            _ => None,
        }
    }
}
fn obj(items: Vec<(&str, T)>) -> T {
    T::Obj(items.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}
fn s(v: &str) -> T {
    T::Str(v.to_owned())
}
fn get<'a>(t: &'a T, path: &[&str]) -> &'a T {
    let mut at = t;
    for key in path {
        at = match at {
            T::Obj(o) => &o.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no {key}")).1,
            _ => panic!("not an object at {key}"),
        };
    }
    at
}

/// The components of vanilla's netherite pickaxe, in the shape of `items.json`.
fn netherite_pickaxe() -> T {
    obj(vec![
        ("minecraft:max_damage", T::Int(2031)),
        ("minecraft:enchantable", obj(vec![("value", T::Int(15))])),
        ("minecraft:repairable", obj(vec![("items", s("minecraft:netherite_ingot"))])),
        (
            "minecraft:attribute_modifiers",
            T::Arr(vec![
                obj(vec![
                    ("type", s("minecraft:attack_damage")),
                    ("id", s("minecraft:base_attack_damage")),
                    ("amount", T::Float(5.0)),
                    ("operation", s("add_value")),
                    ("slot", s("mainhand")),
                ]),
                obj(vec![
                    ("type", s("minecraft:attack_speed")),
                    ("id", s("minecraft:base_attack_speed")),
                    ("amount", T::Float(-2.8)),
                    ("operation", s("add_value")),
                    ("slot", s("mainhand")),
                ]),
            ]),
        ),
        (
            "minecraft:tool",
            obj(vec![
                (
                    "rules",
                    T::Arr(vec![
                        obj(vec![
                            ("blocks", s("#minecraft:incorrect_for_netherite_tool")),
                            ("correct_for_drops", T::Bool(false)),
                        ]),
                        obj(vec![
                            ("blocks", s("#minecraft:mineable/pickaxe")),
                            ("correct_for_drops", T::Bool(true)),
                            ("speed", T::Float(9.0)),
                        ]),
                    ]),
                ),
                ("default_mining_speed", T::Float(1.0)),
            ]),
        ),
    ])
}
fn netherite_helmet() -> T {
    obj(vec![
        ("minecraft:max_damage", T::Int(407)),
        ("minecraft:enchantable", obj(vec![("value", T::Int(15))])),
        ("minecraft:repairable", obj(vec![("items", s("minecraft:netherite_ingot"))])),
        ("minecraft:equippable", obj(vec![("slot", s("head")), ("asset_id", s("minecraft:netherite"))])),
        (
            "minecraft:attribute_modifiers",
            T::Arr(vec![
                obj(vec![("type", s("minecraft:armor")), ("amount", T::Float(3.0))]),
                obj(vec![("type", s("minecraft:armor_toughness")), ("amount", T::Float(3.0))]),
                obj(vec![("type", s("minecraft:knockback_resistance")), ("amount", T::Float(0.1))]),
            ]),
        ),
    ])
}
fn attribute(t: &T, kind: &str) -> f64 {
    let T::Arr(list) = get(t, &["minecraft:attribute_modifiers"]) else {
        panic!("no list")
    };
    for m in list {
        if get(m, &["type"]) == &s(kind) {
            return match get(m, &["amount"]) {
                T::Float(f) => *f,
                other => panic!("amount {other:?}"),
            };
        }
    }
    panic!("no attribute {kind}")
}

// ------------------------------------------------------------------- random

/// Scripted draws: floats, then bounded ints, in the order they are asked.
struct Script {
    floats: Vec<f32>,
    ints: Vec<i32>,
    asked_floats: usize,
    asked_ints: Vec<i32>,
}
impl Script {
    fn new(floats: &[f32], ints: &[i32]) -> Self {
        Self {
            floats: floats.to_vec(),
            ints: ints.to_vec(),
            asked_floats: 0,
            asked_ints: Vec::new(),
        }
    }
}
impl Rng for Script {
    fn next_float(&mut self) -> f32 {
        self.asked_floats += 1;
        self.floats.remove(0)
    }
    fn next_int_bound(&mut self, bound: i32) -> i32 {
        self.asked_ints.push(bound);
        self.ints.remove(0)
    }
}

// ------------------------------------------------------------------- tests

#[test]
fn forty_items_and_four_blocks() {
    let all = items();
    assert_eq!(all.len(), 4 + 16 + 20);
    assert_eq!(blocks().len(), 4);
    let mut ids: Vec<String> = all.iter().map(Spec::id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 40, "ids repeat");
    assert!(ids.contains(&"advancednetherite:netherite_gold_chestplate".to_owned()));
    assert!(ids.contains(&"advancednetherite:netherite_diamond_ingot".to_owned()));
}

#[test]
fn armor_durability_is_health_per_slot_times_the_tier_factor() {
    // ModArmorTiers: HEALTH_PER_SLOT = {13, 15, 16, 11} for feet, legs, chest, head.
    assert_eq!(Tier::Iron.armor_durability(Piece::Helmet), 11 * 39);
    assert_eq!(Tier::Iron.armor_durability(Piece::Chestplate), 16 * 39);
    assert_eq!(Tier::Iron.armor_durability(Piece::Leggings), 15 * 39);
    assert_eq!(Tier::Iron.armor_durability(Piece::Boots), 13 * 39);
    assert_eq!(Tier::Gold.armor_durability(Piece::Chestplate), 16 * 41);
    assert_eq!(Tier::Emerald.armor_durability(Piece::Leggings), 15 * 43);
    assert_eq!(Tier::Diamond.armor_durability(Piece::Helmet), 11 * 47);
    assert_eq!(Tier::Diamond.armor_durability(Piece::Chestplate), 752);
}

#[test]
fn armor_defense_follows_slot_protections() {
    // slotProtections = {4, 6, 8, 4} (iron): boots, leggings, chestplate, helmet.
    assert_eq!(Tier::Iron.armor_defense(Piece::Boots), 4);
    assert_eq!(Tier::Iron.armor_defense(Piece::Leggings), 6);
    assert_eq!(Tier::Iron.armor_defense(Piece::Chestplate), 8);
    assert_eq!(Tier::Iron.armor_defense(Piece::Helmet), 4);
    assert_eq!(Tier::Gold.armor_defense(Piece::Leggings), 7);
    assert_eq!(Tier::Gold.armor_defense(Piece::Chestplate), 9);
    assert_eq!(Tier::Emerald.armor_defense(Piece::Boots), 4);
    assert_eq!(Tier::Diamond.armor_defense(Piece::Boots), 5);
    assert_eq!(Tier::Diamond.armor_defense(Piece::Helmet), 5);
    assert_eq!(Tier::Diamond.armor_toughness(), 4.0);
    assert_eq!(Tier::Iron.armor_toughness(), 3.5);
}

#[test]
fn attack_damage_adds_the_tier_bonus_to_the_constructor_argument() {
    use Tier::*;
    use Tool::*;
    // ModItems: AdvancedAxeItem(tier, 5 / 6 / 6 / 7, -3.0f); the tier bonus is 4 / 4 / 5 / 5.
    assert_eq!(Axe.attack_damage_modifier(Iron), 9.0);
    assert_eq!(Axe.attack_damage_modifier(Gold), 10.0);
    assert_eq!(Axe.attack_damage_modifier(Emerald), 11.0);
    assert_eq!(Axe.attack_damage_modifier(Diamond), 12.0);
    // Hoes: -4 / -4 / -5 / -5, so 0 everywhere.
    for t in TIERS {
        assert_eq!(Hoe.attack_damage_modifier(t), 0.0);
    }
    assert_eq!(Pickaxe.attack_damage_modifier(Iron), 5.0);
    assert_eq!(Pickaxe.attack_damage_modifier(Diamond), 6.0);
    assert_eq!(Shovel.attack_damage_modifier(Iron), 5.5);
    assert_eq!(Shovel.attack_damage_modifier(Gold), 5.5);
    assert_eq!(Shovel.attack_damage_modifier(Emerald), 6.0);
    assert_eq!(Sword.attack_damage_modifier(Iron), 7.0);
    assert_eq!(Sword.attack_damage_modifier(Gold), 8.0);
    assert_eq!(Sword.attack_damage_modifier(Emerald), 9.0);
    assert_eq!(Sword.attack_damage_modifier(Diamond), 10.0);
    assert_eq!(Sword.speed_argument(), -2.4);
    assert_eq!(Pickaxe.speed_argument(), -2.8);
}

#[test]
fn tool_stats_land_on_the_template_components() {
    let settings = Settings::default();
    let ops = patch_for("advancednetherite:netherite_gold_pickaxe", &settings).unwrap();
    let mut t = netherite_pickaxe();
    apply(&mut t, &ops);
    assert_eq!(get(&t, &["minecraft:max_damage"]), &T::Int(2313));
    assert_eq!(get(&t, &["minecraft:enchantable", "value"]), &T::Int(25));
    assert_eq!(
        get(&t, &["minecraft:repairable", "items"]),
        &s("advancednetherite:netherite_gold_ingot")
    );
    assert_eq!(attribute(&t, "minecraft:attack_damage"), 5.0);
    assert_eq!(attribute(&t, "minecraft:attack_speed"), -2.8);
    // The rule that is not correct for drops keeps no speed; the other one is x20.
    let T::Arr(rules) = get(&t, &["minecraft:tool", "rules"]) else {
        panic!()
    };
    assert!(!matches!(&rules[0], T::Obj(o) if o.iter().any(|(k, _)| k == "speed")));
    assert_eq!(get(&rules[1], &["speed"]), &T::Float(20.0));
    // The default mining speed is not touched.
    assert_eq!(get(&t, &["minecraft:tool", "default_mining_speed"]), &T::Float(1.0));
}

#[test]
fn every_diggers_speed_is_the_configured_multiplier() {
    let settings = Settings::default();
    for (tier, speed) in [(Tier::Iron, 12.0), (Tier::Gold, 20.0), (Tier::Emerald, 29.0), (Tier::Diamond, 39.0)] {
        let id = format!("advancednetherite:netherite_{}_pickaxe", tier.name());
        let mut t = netherite_pickaxe();
        apply(&mut t, &patch_for(&id, &settings).unwrap());
        let T::Arr(rules) = get(&t, &["minecraft:tool", "rules"]) else {
            panic!()
        };
        assert_eq!(get(&rules[1], &["speed"]), &T::Float(speed));
    }
    // A sword has no breaking speed change.
    let ops = patch_for("advancednetherite:netherite_iron_sword", &settings).unwrap();
    assert!(!ops.iter().any(|o| matches!(o, Op::ToolSpeed(_))));
}

#[test]
fn armor_stats_land_on_the_template_components() {
    let settings = Settings::default();
    let ops = patch_for("advancednetherite:netherite_diamond_helmet", &settings).unwrap();
    let mut t = netherite_helmet();
    apply(&mut t, &ops);
    assert_eq!(get(&t, &["minecraft:max_damage"]), &T::Int(11 * 47));
    assert_eq!(get(&t, &["minecraft:enchantable", "value"]), &T::Int(15));
    assert_eq!(attribute(&t, "minecraft:armor"), 5.0);
    assert_eq!(attribute(&t, "minecraft:armor_toughness"), 4.0);
    assert_eq!(attribute(&t, "minecraft:knockback_resistance"), 0.1);
    assert_eq!(
        get(&t, &["minecraft:equippable", "asset_id"]),
        &s("advancednetherite:netherite_diamond")
    );
    // The slot stays the template's.
    assert_eq!(get(&t, &["minecraft:equippable", "slot"]), &s("head"));
}

#[test]
fn missing_components_are_made_and_unknown_items_have_no_patch() {
    let settings = Settings::default();
    let mut t = obj(vec![]);
    apply(&mut t, &patch_for("advancednetherite:netherite_iron_boots", &settings).unwrap());
    assert_eq!(get(&t, &["minecraft:max_damage"]), &T::Int(13 * 39));
    assert_eq!(
        get(&t, &["minecraft:equippable", "asset_id"]),
        &s("advancednetherite:netherite_iron")
    );
    assert!(patch_for("advancednetherite:netherite_iron_block", &settings).is_none());
    assert!(patch_for("minecraft:netherite_sword", &settings).is_none());
    // An ingot keeps its template exactly.
    assert_eq!(patch_for("advancednetherite:netherite_iron_ingot", &settings), Some(vec![]));
}

#[test]
fn armor_perks_default_to_the_packs_settings() {
    let st = Settings::default();
    use Pacify::*;
    // Iron: phantoms. Gold: piglins. Emerald: endermen. Diamond: all three.
    assert!(st.pacifies(Tier::Iron, Phantoms) && !st.pacifies(Tier::Iron, Piglins) && !st.pacifies(Tier::Iron, Endermen));
    assert!(!st.pacifies(Tier::Gold, Phantoms) && st.pacifies(Tier::Gold, Piglins) && !st.pacifies(Tier::Gold, Endermen));
    assert!(!st.pacifies(Tier::Emerald, Phantoms) && !st.pacifies(Tier::Emerald, Piglins) && st.pacifies(Tier::Emerald, Endermen));
    assert!(st.pacifies(Tier::Diamond, Phantoms) && st.pacifies(Tier::Diamond, Piglins) && st.pacifies(Tier::Diamond, Endermen));
}

#[test]
fn one_worn_piece_is_enough_and_other_armor_does_nothing() {
    let st = Settings::default();
    let none = worn_perks(&st, [None, None, None, None]);
    assert_eq!(none, Worn::default());
    let vanilla = worn_perks(&st, [Some("minecraft:netherite_helmet"), None, Some("minecraft:diamond_leggings"), None]);
    assert_eq!(vanilla, Worn::default());
    let one = worn_perks(&st, [None, None, None, Some("advancednetherite:netherite_gold_boots")]);
    assert_eq!(one, Worn { phantoms: false, piglins: true, endermen: false });
    // Mixed tiers: the perks add up, as the original asks every piece.
    let mixed = worn_perks(
        &st,
        [
            Some("advancednetherite:netherite_iron_helmet"),
            Some("advancednetherite:netherite_emerald_chestplate"),
            None,
            None,
        ],
    );
    assert_eq!(mixed, Worn { phantoms: true, piglins: false, endermen: true });
    // A tool is not armor.
    let tool = worn_perks(&st, [Some("advancednetherite:netherite_diamond_sword"), None, None, None]);
    assert_eq!(tool, Worn::default());
}

#[test]
fn settings_can_turn_a_perk_off() {
    let mut st = Settings::default();
    st.passive[3] = [false, false, false];
    assert_eq!(
        worn_perks(&st, [Some("advancednetherite:netherite_diamond_helmet")]),
        Worn::default()
    );
}

#[test]
fn item_ids_parse_back() {
    assert_eq!(
        armor_of("advancednetherite:netherite_emerald_leggings"),
        Some((Tier::Emerald, Piece::Leggings))
    );
    assert_eq!(armor_of("advancednetherite:netherite_emerald_sword"), None);
    assert_eq!(armor_of("advancednetherite:netherite_emerald_block"), None);
    assert_eq!(tool_of("advancednetherite:netherite_gold_hoe"), Some((Tier::Gold, Tool::Hoe)));
    assert_eq!(tool_of("minecraft:netherite_hoe"), None);
}

// ---------------------------------------------------------------- drops

#[test]
fn ore_drop_needs_the_matching_pickaxe_and_no_silk_touch() {
    let st = Settings::default();
    // 0.1 <= 0.2: the extra raw iron; nextInt(2) = 1 gives 1 + 1 = 2.
    let mut rng = Script::new(&[0.1], &[1]);
    let d = ore_drops(&st, "advancednetherite:netherite_iron_pickaxe", "minecraft:iron_ore", false, &mut rng);
    assert_eq!(d, vec![Extra { item: "minecraft:raw_iron", count: 2 }]);
    assert_eq!(rng.asked_ints, vec![2]);
    // Deepslate counts too.
    let mut rng = Script::new(&[0.2], &[0]);
    let d = ore_drops(&st, "advancednetherite:netherite_iron_pickaxe", "minecraft:deepslate_iron_ore", false, &mut rng);
    assert_eq!(d, vec![Extra { item: "minecraft:raw_iron", count: 1 }]);
    // Above the chance: nothing, and no count drawn.
    let mut rng = Script::new(&[0.21], &[]);
    let d = ore_drops(&st, "advancednetherite:netherite_iron_pickaxe", "minecraft:iron_ore", false, &mut rng);
    assert!(d.is_empty());
    assert!(rng.asked_ints.is_empty());
    // The gold pickaxe on iron: no draw at all.
    let mut rng = Script::new(&[], &[]);
    assert!(ore_drops(&st, "advancednetherite:netherite_gold_pickaxe", "minecraft:iron_ore", false, &mut rng).is_empty());
    assert_eq!(rng.asked_floats, 0);
    // Silk touch: nothing.
    let mut rng = Script::new(&[], &[]);
    assert!(ore_drops(&st, "advancednetherite:netherite_iron_pickaxe", "minecraft:iron_ore", true, &mut rng).is_empty());
    // A vanilla pickaxe: nothing.
    assert!(ore_drops(&st, "minecraft:netherite_pickaxe", "minecraft:iron_ore", false, &mut Script::new(&[], &[])).is_empty());
}

#[test]
fn every_ore_has_its_pickaxe_chance_and_amount() {
    let st = Settings::default();
    let cases = [
        ("gold", "minecraft:gold_ore", "minecraft:raw_gold", 0.3, 1, 1),
        ("gold", "minecraft:deepslate_gold_ore", "minecraft:raw_gold", 0.3, 1, 1),
        ("emerald", "minecraft:emerald_ore", "minecraft:emerald", 0.4, 1, 1),
        ("emerald", "minecraft:deepslate_emerald_ore", "minecraft:emerald", 0.4, 1, 1),
        ("diamond", "minecraft:diamond_ore", "minecraft:diamond", 0.25, 1, 1),
        ("diamond", "minecraft:deepslate_diamond_ore", "minecraft:diamond", 0.25, 1, 1),
        ("gold", "minecraft:nether_gold_ore", "minecraft:gold_nugget", 0.6, 1, 3),
    ];
    for (tier, ore, item, chance, lo, hi) in cases {
        let tool = format!("advancednetherite:netherite_{tier}_pickaxe");
        let mut rng = Script::new(&[chance as f32], &[0]);
        let d = ore_drops(&st, &tool, ore, false, &mut rng);
        assert_eq!(d, vec![Extra { item, count: lo }], "{ore}");
        assert_eq!(rng.asked_ints, vec![hi - lo + 1], "{ore}");
        let mut rng = Script::new(&[chance as f32 + 0.01], &[0]);
        assert!(ore_drops(&st, &tool, ore, false, &mut rng).is_empty(), "{ore}");
    }
}

#[test]
fn crops_only_drop_more_for_a_mod_hoe_and_when_grown() {
    let st = Settings::default();
    let hoe = "advancednetherite:netherite_diamond_hoe";
    // Wheat: 0.3 chance, 0..2 more.
    let mut rng = Script::new(&[0.3], &[2]);
    assert_eq!(
        crop_drops(&st, hoe, "minecraft:wheat", true, &mut rng),
        vec![Extra { item: "minecraft:wheat", count: 2 }]
    );
    assert_eq!(rng.asked_ints, vec![3]);
    // nextIntBetweenInclusive may give 0: then nothing is added.
    let mut rng = Script::new(&[0.0], &[0]);
    assert!(crop_drops(&st, hoe, "minecraft:wheat", true, &mut rng).is_empty());
    // Not grown, wrong tool, a pickaxe: no draw.
    for (tool, mature) in [(hoe, false), ("minecraft:netherite_hoe", true), ("advancednetherite:netherite_iron_pickaxe", true)] {
        let mut rng = Script::new(&[], &[]);
        assert!(crop_drops(&st, tool, "minecraft:wheat", mature, &mut rng).is_empty());
        assert_eq!(rng.asked_floats, 0);
    }
    // Potatoes 0..1, beetroots 1..2 with a 0.2 chance, carrots 0..2.
    let mut rng = Script::new(&[0.3], &[1]);
    assert_eq!(
        crop_drops(&st, hoe, "minecraft:potatoes", true, &mut rng),
        vec![Extra { item: "minecraft:potato", count: 1 }]
    );
    assert_eq!(rng.asked_ints, vec![2]);
    let mut rng = Script::new(&[0.2], &[1]);
    assert_eq!(
        crop_drops(&st, hoe, "minecraft:beetroots", true, &mut rng),
        vec![Extra { item: "minecraft:beetroot", count: 2 }]
    );
    let mut rng = Script::new(&[0.21], &[]);
    assert!(crop_drops(&st, hoe, "minecraft:beetroots", true, &mut rng).is_empty());
    let mut rng = Script::new(&[0.3], &[2]);
    assert_eq!(
        crop_drops(&st, hoe, "minecraft:carrots", true, &mut rng),
        vec![Extra { item: "minecraft:carrot", count: 2 }]
    );
    // Any other block: nothing.
    assert!(crop_drops(&st, hoe, "minecraft:stone", true, &mut Script::new(&[], &[])).is_empty());
}

#[test]
fn crop_age_limits() {
    assert!(crop_is_mature("minecraft:wheat", Some(7)));
    assert!(!crop_is_mature("minecraft:wheat", Some(6)));
    assert!(crop_is_mature("minecraft:beetroots", Some(3)));
    assert!(!crop_is_mature("minecraft:beetroots", Some(2)));
    assert!(!crop_is_mature("minecraft:wheat", None));
    assert!(!crop_is_mature("minecraft:pumpkin_stem", Some(7)));
}

#[test]
fn mob_drops_follow_the_sword_and_the_victim() {
    let st = Settings::default();
    // Iron sword, phantom: 0.5 chance, 0..2 membranes.
    let mut rng = Script::new(&[0.5], &[2]);
    assert_eq!(
        mob_drops(&st, "advancednetherite:netherite_iron_sword", "minecraft:phantom", &mut rng),
        vec![Extra { item: "minecraft:phantom_membrane", count: 2 }]
    );
    // Gold and diamond swords: zombified piglin gold nuggets (0..3), piglin one ingot (0.15).
    for sword in ["gold", "diamond"] {
        let tool = format!("advancednetherite:netherite_{sword}_sword");
        let mut rng = Script::new(&[0.5], &[3]);
        assert_eq!(
            mob_drops(&st, &tool, "minecraft:zombified_piglin", &mut rng),
            vec![Extra { item: "minecraft:gold_nugget", count: 3 }]
        );
        assert_eq!(rng.asked_ints, vec![4]);
        let mut rng = Script::new(&[0.15], &[0]);
        assert_eq!(
            mob_drops(&st, &tool, "minecraft:piglin", &mut rng),
            vec![Extra { item: "minecraft:gold_ingot", count: 1 }]
        );
        let mut rng = Script::new(&[0.16], &[]);
        assert!(mob_drops(&st, &tool, "minecraft:piglin", &mut rng).is_empty());
    }
    // Emerald and diamond swords: enderman pearls (0.3, 0..1).
    for sword in ["emerald", "diamond"] {
        let tool = format!("advancednetherite:netherite_{sword}_sword");
        let mut rng = Script::new(&[0.3], &[1]);
        assert_eq!(
            mob_drops(&st, &tool, "minecraft:enderman", &mut rng),
            vec![Extra { item: "minecraft:ender_pearl", count: 1 }]
        );
    }
    // Each sword only for its victims: no draw otherwise.
    for (sword, victim) in [
        ("iron", "minecraft:piglin"),
        ("iron", "minecraft:enderman"),
        ("gold", "minecraft:phantom"),
        ("gold", "minecraft:enderman"),
        ("emerald", "minecraft:piglin"),
        ("emerald", "minecraft:phantom"),
        ("diamond", "minecraft:phantom"),
        ("diamond", "minecraft:zombie"),
    ] {
        let tool = format!("advancednetherite:netherite_{sword}_sword");
        let mut rng = Script::new(&[], &[]);
        let d = mob_drops(&st, &tool, victim, &mut rng);
        // diamond on phantom: no, the original lists the iron sword only.
        assert!(d.is_empty(), "{sword} on {victim}");
        assert_eq!(rng.asked_floats, 0, "{sword} on {victim}");
    }
    // An axe in hand: nothing.
    assert!(mob_drops(&st, "advancednetherite:netherite_gold_axe", "minecraft:piglin", &mut Script::new(&[], &[])).is_empty());
}

#[test]
fn the_extra_drops_can_be_turned_off() {
    let mut st = Settings::default();
    st.additional_mob_drops = false;
    st.additional_ore_drops = false;
    st.additional_crop_drops = false;
    assert!(mob_drops(&st, "advancednetherite:netherite_iron_sword", "minecraft:phantom", &mut Script::new(&[], &[])).is_empty());
    assert!(ore_drops(&st, "advancednetherite:netherite_iron_pickaxe", "minecraft:iron_ore", false, &mut Script::new(&[], &[])).is_empty());
    assert!(crop_drops(&st, "advancednetherite:netherite_iron_hoe", "minecraft:wheat", true, &mut Script::new(&[], &[])).is_empty());
}

// --------------------------------------------- the carried content agrees

fn carry() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/mods/advancednetherite/carry")
}

fn json_files(dir: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .filter(|n| n.ends_with(".json"))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

#[test]
fn the_carried_declarations_are_exactly_these_items_and_blocks() {
    let root = carry().join("data/advancednetherite/horizon");
    if !root.exists() {
        eprintln!("no carry folder here, skipped");
        return;
    }
    let mut want: Vec<String> = items().iter().map(|i| format!("{}.json", i.path)).collect();
    want.sort();
    assert_eq!(json_files(&root.join("item")), want, "data/advancednetherite/horizon/item");
    for spec in items() {
        let text = std::fs::read_to_string(root.join("item").join(format!("{}.json", spec.path))).unwrap();
        assert!(
            text.contains(&format!("\"template\": \"{}\"", spec.template)),
            "{}: template should be {}",
            spec.path,
            spec.template
        );
    }
    let mut want: Vec<String> = blocks().iter().map(|(p, _)| format!("{p}.json")).collect();
    want.sort();
    assert_eq!(json_files(&root.join("block")), want);
    for (path, template) in blocks() {
        let text = std::fs::read_to_string(root.join("block").join(format!("{path}.json"))).unwrap();
        assert!(text.contains(&format!("\"template\": \"{template}\"")), "{path}");
    }
}

#[test]
fn describe_lists_each_tier() {
    let d = describe(&Settings::default());
    assert_eq!(d.lines().count(), 4);
    assert!(d.contains("diamond: phantoms true piglins true endermen true, speed x39"));
}
