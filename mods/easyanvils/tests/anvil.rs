use horizon_easyanvils as e;
use e::{Config, Rules, Stack, Store, Wear};

const CFG: Config = Config::MARIAN;

struct R;
impl Rules for R {
    fn can_enchant(&self, id: &str, item: &str) -> bool {
        match id {
            "sharpness" | "smite" => item.ends_with("_sword") || item.ends_with("_axe"),
            "unbreaking" | "mending" => item.ends_with("_sword") || item.ends_with("_pickaxe"),
            "silk_touch" | "fortune" => item.ends_with("_pickaxe"),
            _ => false,
        }
    }
    fn compatible(&self, a: &str, b: &str) -> bool {
        let pair = |x: &str, y: &str| (a == x && b == y) || (a == y && b == x);
        a != b && !pair("silk_touch", "fortune") && !pair("sharpness", "smite")
    }
    fn max_level(&self, id: &str) -> i32 {
        match id {
            "sharpness" | "smite" => 5,
            "unbreaking" => 3,
            _ => 1,
        }
    }
    fn anvil_cost(&self, id: &str) -> i32 {
        match id {
            "sharpness" => 1,
            "smite" => 2,
            "unbreaking" => 2,
            "mending" => 4,
            "silk_touch" => 8,
            "fortune" => 4,
            _ => 1,
        }
    }
    fn valid_repair(&self, stack: &Stack, material: &Stack) -> bool {
        stack.item == "minecraft:iron_sword" && material.item == "minecraft:iron_ingot"
    }
}

fn sword() -> Stack {
    Stack {
        item: "minecraft:iron_sword".into(),
        count: 1,
        damage: 0,
        max_damage: 250,
        repair_cost: 0,
        custom_name: None,
        enchants: vec![],
    }
}
fn book(ench: &[(&str, i32)]) -> Stack {
    Stack {
        item: "minecraft:enchanted_book".into(),
        count: 1,
        damage: 0,
        max_damage: 0,
        repair_cost: 0,
        custom_name: None,
        enchants: ench.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    }
}
fn res(left: &Stack, right: Option<&Stack>, name: Option<&str>, creative: bool) -> e::Out {
    e::anvil_result(&R, &CFG, Some(left), right, name, "Iron Sword", creative)
}

#[test]
fn tuned_config_values() {
    assert_eq!(CFG.anvil_break_chance, 0.05);
    assert!(CFG.risk_free_anvil_renaming && CFG.edit_name_tags_no_anvil && CFG.anvil_repairing);
    assert_eq!(CFG.too_expensive_limit, -1);
    assert_eq!(CFG.free_renames, e::FreeRenames::AllItems);
    assert_eq!(CFG.rename_and_repair_costs, e::RenameAndRepairCost::Fixed);
    assert_eq!(CFG.prior_work_penalty, e::PriorWorkPenalty::Fixed);
    assert_eq!(CFG.prior_work_penalty_constant, 4);
    assert!(CFG.penalty_free_renames_and_repairs && CFG.halved_book_costs);
    assert_eq!(
        (CFG.common_multiplier, CFG.uncommon_multiplier, CFG.rare_multiplier, CFG.very_rare_multiplier),
        (1, 2, 4, 8)
    );
    assert_eq!((CFG.repair_with_material_unit_cost, CFG.repair_with_other_item_cost), (1, 2));
}

#[test]
fn prior_work_counts_the_times_an_item_was_worked() {
    assert_eq!(e::repair_cost_to_repairs(0), 0);
    assert_eq!(e::repair_cost_to_repairs(1), 1);
    assert_eq!(e::repair_cost_to_repairs(3), 2);
    assert_eq!(e::repair_cost_to_repairs(7), 3);
    assert_eq!(e::prior_work(&CFG, 7), 12);
    assert_eq!(e::increased_repair_cost(0), 1);
    assert_eq!(e::increased_repair_cost(3), 7);
}

#[test]
fn renaming_is_free_and_keeps_the_penalty() {
    let mut s = sword();
    s.repair_cost = 7;
    let out = res(&s, None, Some("Excalibur"), false);
    assert_eq!(out.cost, 0);
    let r = out.result.expect("a free rename still gives the item");
    assert_eq!(r.custom_name.as_deref(), Some("Excalibur"));
    // penalty_free_renames_and_repairs: the stored cost is not raised
    assert_eq!(r.repair_cost, 7);
    assert!(e::may_pickup(false, 0, out.cost));
    // untouched name: no result
    assert!(res(&s, None, Some("Iron Sword"), false).result.is_none());
    assert!(res(&s, None, None, false).result.is_none());
}

#[test]
fn removing_a_name_is_free_too() {
    let mut s = sword();
    s.custom_name = Some("Excalibur".into());
    let out = res(&s, None, Some(""), false);
    assert_eq!(out.cost, 0);
    assert_eq!(out.result.unwrap().custom_name, None);
    // formatting codes alone are an empty name
    assert!(res(&s, None, Some("§a§l"), false).result.unwrap().custom_name.is_none());
}

#[test]
fn a_book_is_halved_and_the_penalty_grows() {
    let s = sword();
    let b = book(&[("sharpness", 3)]);
    let out = res(&s, Some(&b), None, false);
    // sharpness is common (1): halved books max(1, 0) = 1 per level
    assert_eq!(out.cost, 3);
    let r = out.result.unwrap();
    assert_eq!(r.enchants, vec![("sharpness".to_string(), 3)]);
    assert_eq!(r.repair_cost, 1);
    // a rare enchantment on a book: mending 4 -> 2 per level
    let out = res(&s, Some(&book(&[("mending", 1)])), None, false);
    assert_eq!(out.cost, 2);
    // very rare: silk touch 8 -> 4
    let mut pick = sword();
    pick.item = "minecraft:iron_pickaxe".into();
    let out = e::anvil_result(&R, &CFG, Some(&pick), Some(&book(&[("silk_touch", 1)])), None, "x", false);
    assert_eq!(out.cost, 4);
}

#[test]
fn an_unbooked_item_pays_the_full_multiplier() {
    let mut s = sword();
    s.enchants = vec![("unbreaking".into(), 2)];
    let mut other = sword();
    other.enchants = vec![("unbreaking".into(), 2)];
    let out = res(&s, Some(&other), None, false);
    // two equal levels make 3; uncommon multiplier 2 -> 6; both swords undamaged: no repair
    assert_eq!(out.cost, 6);
    assert_eq!(out.result.unwrap().enchants, vec![("unbreaking".to_string(), 3)]);
}

#[test]
fn prior_work_adds_a_fixed_amount_per_time_worked() {
    let mut s = sword();
    s.repair_cost = 3; // worked twice
    let b = book(&[("sharpness", 3)]);
    let out = res(&s, Some(&b), None, false);
    // repairCostToRepairs(3) = 2, times 4 = 8, plus 3
    assert_eq!(out.cost, 11);
    // a pure repair ignores it (FIXED rename_and_repair_costs)
    let mut worn = s.clone();
    worn.damage = 100;
    let ingot = Stack { item: "minecraft:iron_ingot".into(), count: 4, damage: 0, max_damage: 0, repair_cost: 0, custom_name: None, enchants: vec![] };
    let out = res(&worn, Some(&ingot), None, false);
    // each ingot repairs a quarter (62): two ingots (100 -> 38 -> 0): 2 levels
    assert_eq!(out.cost, 2);
    assert_eq!(out.repair_item_count_cost, 2);
    let r = out.result.unwrap();
    assert_eq!(r.damage, 0);
    // a repair does not raise the stored penalty
    assert_eq!(r.repair_cost, 3);
}

#[test]
fn there_is_no_too_expensive_limit() {
    let mut s = sword();
    s.repair_cost = 1023; // worked ten times: 10 * 4 = 40
    s.enchants = vec![("sharpness".into(), 1)];
    let b = book(&[("mending", 1), ("unbreaking", 3)]);
    let out = res(&s, Some(&b), None, false);
    assert!(out.cost >= 40, "{}", out.cost);
    assert!(out.result.is_some());
    assert!(e::may_pickup(false, out.cost, out.cost));
    assert!(!e::may_pickup(false, out.cost - 1, out.cost));
}

#[test]
fn a_stack_of_several_cannot_take_enchantments() {
    let mut s = sword();
    s.count = 2;
    let out = res(&s, Some(&book(&[("sharpness", 1)])), None, false);
    assert!(out.result.is_none());
    assert_eq!(out.cost, -1);
    assert!(!e::may_pickup(true, 100, out.cost));
    // creative may
    assert!(res(&s, Some(&book(&[("sharpness", 1)])), None, true).result.is_some());
}

#[test]
fn incompatible_enchantments_do_not_combine() {
    let mut pick = sword();
    pick.item = "minecraft:iron_pickaxe".into();
    pick.enchants = vec![("silk_touch".into(), 1)];
    let out = e::anvil_result(&R, &CFG, Some(&pick), Some(&book(&[("fortune", 3)])), None, "x", false);
    assert!(out.result.is_none());
    assert_eq!(out.cost, 0);
    // the unsupported one for the item
    let out = res(&sword(), Some(&book(&[("fortune", 1)])), None, false);
    assert!(out.result.is_none());
}

#[test]
fn a_level_above_the_maximum_is_kept_not_lowered() {
    let mut s = sword();
    s.enchants = vec![("sharpness".into(), 6)];
    let mut o = sword();
    o.enchants = vec![("sharpness".into(), 5)];
    assert!(res(&s, Some(&o), None, false).result.is_none());
    // equal maximum levels do not rise past the maximum, and cost nothing
    let mut a = sword();
    a.enchants = vec![("sharpness".into(), 5)];
    assert!(res(&a, Some(&a.clone()), None, false).result.is_none());
}

#[test]
fn two_damaged_tools_merge_with_a_twelve_percent_bonus() {
    let mut a = sword();
    a.damage = 200;
    let mut b = sword();
    b.damage = 100;
    let out = res(&a, Some(&b), None, false);
    // (250-200) + (250-100) + 250*12/100 = 50 + 150 + 30 = 230 -> damage 20; costs 2
    assert_eq!(out.cost, 2);
    assert_eq!(out.result.unwrap().damage, 20);
}

#[test]
fn material_count_is_used_for_the_right_slot() {
    let mut worn = sword();
    worn.damage = 40;
    let ingot = Stack { item: "minecraft:iron_ingot".into(), count: 9, damage: 0, max_damage: 0, repair_cost: 0, custom_name: None, enchants: vec![] };
    let out = res(&worn, Some(&ingot), None, false);
    assert_eq!(out.repair_item_count_cost, 1);
    assert_eq!(out.result.unwrap().damage, 0);
    // nothing to repair: no result
    assert!(res(&sword(), Some(&ingot), None, false).result.is_none());
}

#[test]
fn rename_with_enchanting_pays_nothing_extra_for_the_name() {
    let s = sword();
    let b = book(&[("sharpness", 2)]);
    let out = res(&s, Some(&b), Some("Blade"), false);
    // 2 for the book; the rename is free
    assert_eq!(out.cost, 2);
    assert_eq!(out.result.unwrap().custom_name.as_deref(), Some("Blade"));
}

#[test]
fn text_helpers() {
    assert_eq!(e::formatted_length("§aHi§r"), 2);
    assert_eq!(e::formatted_length("§"), 1);
    assert_eq!(e::formatted_length("§zq"), 3);
    assert_eq!(e::formatted_length(""), 0);
    assert_eq!(e::filter_text("a\u{7}b§c\n"), "ab§c");
}

#[test]
fn wear_uses_the_config_chance_and_is_risk_free_for_renaming() {
    // 5 percent
    assert_eq!(e::wear(&CFG, "minecraft:anvil", false, false, 0.04), Wear::Becomes("minecraft:chipped_anvil"));
    assert_eq!(e::wear(&CFG, "minecraft:anvil", false, false, 0.06), Wear::None);
    assert_eq!(e::wear(&CFG, "minecraft:chipped_anvil", false, false, 0.0), Wear::Becomes("minecraft:damaged_anvil"));
    assert_eq!(e::wear(&CFG, "minecraft:damaged_anvil", false, false, 0.0), Wear::Break);
    // a rename alone (right slot empty) never wears it
    assert_eq!(e::wear(&CFG, "minecraft:damaged_anvil", true, false, 0.0), Wear::None);
    // creative never
    assert_eq!(e::wear(&CFG, "minecraft:anvil", false, true, 0.0), Wear::None);
    // not an anvil
    assert_eq!(e::wear(&CFG, "minecraft:stone", false, false, 0.0), Wear::None);
    let vanilla_risky = Config { risk_free_anvil_renaming: false, ..CFG };
    assert_ne!(e::wear(&vanilla_risky, "minecraft:anvil", true, false, 0.0), Wear::None);
}

#[test]
fn iron_block_repairs_one_stage() {
    assert_eq!(e::repaired("minecraft:damaged_anvil"), Some("minecraft:chipped_anvil"));
    assert_eq!(e::repaired("minecraft:chipped_anvil"), Some("minecraft:anvil"));
    assert_eq!(e::repaired("minecraft:anvil"), None);
    assert_eq!(e::repaired("minecraft:stone"), None);
}

#[test]
fn the_items_stay_in_the_anvil() {
    let p = [3, 70, 3];
    let mut s: Store<String> = Store::default();
    let slots = s.open(p);
    assert_eq!(slots, vec![None, None]);
    assert!(s.mirror(p, &[Some("sword".into()), Some("book".into())]));
    assert!(!s.mirror(p, &[Some("sword".into()), Some("book".into())]));
    assert_eq!(s.open(p)[1].as_deref(), Some("book"));
    // the anvil wears (still an anvil): kept; it breaks: spilled once
    assert!(s.take_gone(|_| Some(true)).is_empty());
    assert!(s.take_gone(|_| None).is_empty());
    let gone = s.take_gone(|_| Some(false));
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].1[0].as_deref(), Some("sword"));
    assert!(s.take_gone(|_| Some(false)).is_empty());
}

#[test]
fn the_world_file_round_trips() {
    let mut s: Store<String> = Store::default();
    s.mirror([1, 2, 3], &[Some("{id:\"minecraft:iron_sword\",count:1}".into()), None]);
    s.mirror([4, 5, 6], &[None, Some("{id:\"minecraft:book\",count:3}".into())]);
    s.mirror([7, 8, 9], &[None, None]);
    let text = e::save(&s, |x| x.clone());
    assert!(!text.contains("7\t8\t9"));
    let back: Store<String> = e::load(&text, |x| Some(x.to_owned()));
    assert_eq!(back.anvils.len(), 2);
    assert_eq!(back.anvils[&[4, 5, 6]][1].as_deref(), Some("{id:\"minecraft:book\",count:3}"));
    let junk: Store<String> = e::load("S\t0\tq\nA\tx\ty\nA\t1\t2\t3\nS\t5\tz\nS\t1\tw\n", |x| Some(x.to_owned()));
    assert_eq!(junk.anvils[&[1, 2, 3]][1].as_deref(), Some("w"));
}

#[test]
fn the_items_lie_on_the_top_of_the_anvil() {
    let p = e::flat_pose("north", 0).unwrap();
    assert_eq!((p.lift, p.scale), ([0.0, 1.0375, 0.0], 0.375));
    // north (step -1), index 0: mirrored = (0 != 0) = false -> Z axis, not mirrored
    assert_eq!((p.spin_z, p.shift), (90.0, [-0.75, -0.5, 0.0]));
    let q = e::flat_pose("north", 1).unwrap();
    assert_eq!((q.spin_z, q.shift), (-90.0, [0.25, 0.5, 0.0]));
    // east (X axis, step +1), index 0: mirrored = (1 != 0) = true
    let r = e::flat_pose("east", 0).unwrap();
    assert_eq!((r.spin_z, r.shift), (0.0, [0.25, -0.5, 0.0]));
    let s = e::flat_pose("east", 1).unwrap();
    assert_eq!((s.spin_z, s.shift), (180.0, [-0.75, 0.5, 0.0]));
    assert!(e::flat_pose("up", 0).is_none());
}

#[test]
fn one_player_at_a_time_takes_an_anvils_slots() {
    let (p, q) = ([1, 64, 1], [2, 64, 1]);
    // (position, who): 0 = the in-frame host, 7 = a server guest's connection
    let holders = vec![(p, 0u32)];
    assert!(!e::may_bind(&holders, p, &7, true), "the host holds it: the guest gets a plain anvil");
    assert!(e::may_bind(&holders, p, &0, true));
    assert!(e::may_bind(&holders, q, &7, true));
    assert!(!e::may_bind(&holders, q, &7, false), "slots with the player's own items stay his");
}

#[test]
fn the_anvils_a_server_sends_its_clients_round_trip() {
    // A server's client draws the anvils from the world file's text its server sends (`easyanvils:anvils`).
    let mut s: Store<String> = Store::default();
    s.mirror([3, 70, -4], &[Some("{id:\"minecraft:diamond_sword\",count:1}".into()), None]);
    s.mirror([9, 70, 9], &[None, None]);
    let back: Store<String> = e::load(&e::save(&s, |x| x.clone()), |x| Some(x.to_owned()));
    assert_eq!(back.anvils.len(), 1, "empty anvils are not sent");
    assert_eq!(back.anvils[&[3, 70, -4]], s.anvils[&[3, 70, -4]]);
}
