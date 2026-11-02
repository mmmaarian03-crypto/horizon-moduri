// The Storage Drawers rules on a fake world (values from Storage Drawers 1.20 source and
// the pack's storagedrawers-common.v2.toml).
use horizon_storagedrawers::*;
use std::collections::HashMap;

fn cfg() -> Config {
    Config::tuned()
}
fn block(name: &str) -> Block {
    match kind(name, &cfg()) {
        Kind::Drawer(b) => b,
        other => panic!("{name}: {other:?}"),
    }
}

#[test]
fn block_names_and_sizes() {
    let c = cfg();
    // Slots plus the 7 upgrade slots.
    assert_eq!(chest_size("storagedrawers:oak_full_drawers_1", &c), Some(8));
    assert_eq!(chest_size("storagedrawers:spruce_half_drawers_2", &c), Some(9));
    assert_eq!(chest_size("storagedrawers:warped_full_drawers_4", &c), Some(11));
    assert_eq!(chest_size("storagedrawers:framed_full_drawers_4", &c), Some(11));
    assert_eq!(chest_size("storagedrawers:compacting_drawers_3", &c), Some(10));
    assert_eq!(chest_size("storagedrawers:framed_compacting_half_drawers_2", &c), Some(9));
    assert_eq!(chest_size("storagedrawers:controller", &c), None);
    assert_eq!(kind("storagedrawers:controller", &c), Kind::Controller);
    assert_eq!(kind("storagedrawers:controller_slave", &c), Kind::Networked);
    assert_eq!(kind("storagedrawers:controller_io", &c), Kind::Networked);
    assert_eq!(kind("storagedrawers:oak_trim", &c), Kind::Networked);
    assert_eq!(kind("storagedrawers:framed_trim", &c), Kind::Networked);
    assert_eq!(kind("storagedrawers:framing_table", &c), Kind::Other);
    assert_eq!(kind("storagedrawers:oak_full_drawers_3", &c), Kind::Other);
    assert_eq!(kind("storagedrawers:stone_full_drawers_1", &c), Kind::Other);
    assert_eq!(kind("minecraft:chest", &c), Kind::Other);
    // Every wood and size the mod registers.
    for w in [
        "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "bamboo",
        "crimson", "warped",
    ] {
        for d in ["full", "half"] {
            for n in [1, 2, 4] {
                assert!(matches!(
                    kind(&format!("storagedrawers:{w}_{d}_drawers_{n}"), &c),
                    Kind::Drawer(_)
                ));
            }
        }
    }
}

#[test]
fn units_per_slot_are_tuned() {
    // storagedrawers-common.v2.toml: FullStorage1x1 32, 1x2 16, 2x2 8, FullCompacting 32,
    // HalfStorage1x1 16, 1x2 8, 2x2 4, HalfCompacting 16.
    assert_eq!(block("storagedrawers:oak_full_drawers_1").units, 32);
    assert_eq!(block("storagedrawers:oak_full_drawers_2").units, 16);
    assert_eq!(block("storagedrawers:oak_full_drawers_4").units, 8);
    assert_eq!(block("storagedrawers:oak_half_drawers_1").units, 16);
    assert_eq!(block("storagedrawers:oak_half_drawers_2").units, 8);
    assert_eq!(block("storagedrawers:oak_half_drawers_4").units, 4);
    assert_eq!(block("storagedrawers:compacting_drawers_3").units, 32);
    assert_eq!(block("storagedrawers:compacting_half_drawers_2").units, 16);
}

#[test]
fn capacities_follow_the_upgrades() {
    let c = cfg();
    let b = block("storagedrawers:oak_full_drawers_1"); // 32 stacks
    assert_eq!(slot_capacity(&b, &[], &c, 64), 2048);
    assert_eq!(slot_capacity(&b, &[], &c, 16), 512);
    assert_eq!(slot_capacity(&b, &[], &c, 1), 32);
    // Multipliers add: obsidian 2 + copper 4 = 6.
    let ups = [Upgrade::Storage(1), Upgrade::Storage(2)];
    assert_eq!(storage_multiplier(&ups, &c), 6);
    assert_eq!(slot_capacity(&b, &ups, &c, 64), 2048 * 6);
    // Each tier alone: 2, 4, 8, 16, 64, 256, 2048.
    for (l, m) in [(1, 2), (2, 4), (3, 8), (4, 16), (5, 64), (6, 256), (7, 2048)] {
        assert_eq!(storage_multiplier(&[Upgrade::Storage(l)], &c), m);
    }
    // One-stack: one stack of the item.
    assert_eq!(slot_capacity(&b, &[Upgrade::OneStack], &c, 64), 64);
    // Netherite alone: 64 * 32 * 2048 items.
    assert_eq!(slot_capacity(&b, &[Upgrade::Storage(7)], &c, 64), 4_194_304);
    // Saturates at Integer.MAX_VALUE.
    let mut big = cfg();
    big.base_stack_storage = 1000;
    assert_eq!(slot_capacity(&b, &[Upgrade::Storage(7)], &big, 64), MAX_COUNT);
    // Creative storage / vending.
    assert_eq!(slot_capacity(&b, &[Upgrade::CreativeStorage], &c, 64), MAX_COUNT);
    // The quarter drawer: 8 stacks.
    let q = block("storagedrawers:oak_full_drawers_4");
    assert_eq!(slot_capacity(&q, &[], &c, 64), 512);
    let mut c2 = cfg();
    c2.base_stack_storage = 2;
    assert_eq!(slot_capacity(&q, &[], &c2, 64), 1024);
    // One-stack ignores the base value.
    assert_eq!(slot_capacity(&q, &[Upgrade::OneStack], &c2, 64), 64);
}

#[test]
fn upgrade_items_and_groups() {
    assert_eq!(
        Upgrade::parse("storagedrawers:netherite_storage_upgrade"),
        Some(Upgrade::Storage(7))
    );
    assert_eq!(Upgrade::parse("storagedrawers:remote_upgrade_bound"), Some(Upgrade::Remote));
    assert_eq!(Upgrade::parse("storagedrawers:drawer_key"), None);
    assert_eq!(Upgrade::parse("minecraft:iron_ingot"), None);
    // Several storage tiers and magnets; one of anything else.
    let have = [Upgrade::Storage(1), Upgrade::Magnet(1), Upgrade::Void, Upgrade::Push(1)];
    assert!(can_add_upgrade(&have, Upgrade::Storage(3)));
    assert!(can_add_upgrade(&have, Upgrade::Magnet(2)));
    assert!(!can_add_upgrade(&have, Upgrade::Void));
    assert!(!can_add_upgrade(&have, Upgrade::Push(3)));
    assert!(can_add_upgrade(&have, Upgrade::Pull(1)));
    assert!(!can_add_upgrade(&[Upgrade::OneStack], Upgrade::OneStack));
    assert_eq!(
        upgrades_of(&[Some("storagedrawers:void_upgrade"), None, Some("minecraft:dirt")]),
        vec![Upgrade::Void]
    );
}

#[test]
fn insert_and_extract() {
    // 100 of 120, 50 offered: 20 fit.
    assert_eq!(insert(100, 120, 50, &[]), Insert { consumed: 20, stored: 20 });
    // Void: the whole stack goes, the surplus is lost.
    assert_eq!(insert(100, 120, 50, &[Upgrade::Void]), Insert { consumed: 50, stored: 20 });
    // Vending: everything is swallowed and nothing stored (putItemsIntoSlot).
    assert_eq!(
        insert(5, 50, 7, &[Upgrade::CreativeVending]),
        Insert { consumed: 7, stored: 0 }
    );
    assert_eq!(insert(120, 120, 5, &[]), Insert { consumed: 0, stored: 0 });
    assert_eq!(extract(10, 64, &[]), (10, 0));
    assert_eq!(extract(100, 64, &[]), (64, 36));
    // Vending keeps its count and always gives.
    assert_eq!(extract(1, 64, &[Upgrade::CreativeVending]), (64, 1));
}

#[test]
fn what_a_slot_accepts() {
    // Empty, unlocked: anything. Empty, locked: players only. Populated: the same item.
    assert!(can_store(false, false, false, false));
    assert!(!can_store(false, false, true, false));
    assert!(can_store(false, false, true, true));
    assert!(can_store(true, true, true, false));
    assert!(!can_store(true, false, false, true));
    let c = cfg();
    assert!(c.denied("storagedrawers:creative_vending_upgrade"));
    assert!(!c.denied("minecraft:dirt"));
    let mut c2 = cfg();
    c2.store_deny.push("somemod".to_owned());
    assert!(c2.denied("somemod:thing"));
}

#[test]
fn compacting_chains() {
    let r = rules(&cfg());
    // Iron: block (81), ingot (9), nugget (1).
    let ch = chain_around("minecraft:iron_ingot", 3, &r);
    assert_eq!(
        ch.items,
        vec!["minecraft:iron_block", "minecraft:iron_ingot", "minecraft:iron_nugget"]
    );
    assert_eq!(ch.rates, vec![81, 9, 1]);
    // The same chain from any tier.
    assert_eq!(chain_around("minecraft:iron_block", 3, &r), ch);
    assert_eq!(chain_around("minecraft:iron_nugget", 3, &r), ch);
    assert_eq!(chain_from_smallest("minecraft:iron_nugget", 3, &r), ch);
    // Two tiers: the item and one neighbour; the smallest of the two is the pool unit.
    let two = chain_around("minecraft:iron_block", 2, &r);
    assert_eq!(two.items, vec!["minecraft:iron_block", "minecraft:iron_ingot"]);
    assert_eq!(two.rates, vec![9, 1]);
    assert_eq!(chain_from_smallest("minecraft:iron_ingot", 2, &r), two);
    // Diamond has no nugget: two tiers even in a three-slot drawer.
    let d = chain_around("minecraft:diamond", 3, &r);
    assert_eq!(d.items, vec!["minecraft:diamond_block", "minecraft:diamond"]);
    assert_eq!(d.rates, vec![9, 1]);
    // The config's clay rule and the extra rules: 4 clay balls.
    let cl = chain_around("minecraft:clay_ball", 2, &r);
    assert_eq!(cl.rates, vec![4, 1]);
    // Cobblestone has no tiers: a plain one-slot drawer.
    let s = chain_around("minecraft:cobblestone", 3, &r);
    assert_eq!(s.items, vec!["minecraft:cobblestone"]);
    assert_eq!(s.rates, vec![1]);
    assert_eq!(s.smallest(), 0);
}

#[test]
fn compacting_pool() {
    let c = cfg();
    let r = rules(&c);
    let b = block("storagedrawers:compacting_drawers_3"); // 32 units
    let ch = chain_around("minecraft:iron_ingot", 3, &r);
    // 64 blocks * 32 stacks * 81 nuggets each.
    let cap = pool_capacity(&ch, &[], &b, &c, 64);
    assert_eq!(cap, 64 * 32 * 81);
    // 5 ingots go in as 45 nuggets; the counts shown are pool / rate.
    let (used, pool) = pool_insert(0, cap, 9, 5, &[]);
    assert_eq!((used, pool), (5, 45));
    assert_eq!(pool / ch.rates[0], 0);
    assert_eq!(pool / ch.rates[1], 5);
    let (used, pool) = pool_insert(pool, cap, 81, 2, &[]);
    assert_eq!((used, pool), (2, 45 + 162));
    // Take 3 ingots.
    assert_eq!(pool_take(pool, 9, 3, &[]), (3, pool - 27));
    // More than there is.
    assert_eq!(pool_take(10, 9, 5, &[]), (1, 1));
    // Full pool: the surplus is refused; void swallows it.
    assert_eq!(pool_insert(cap - 3, cap, 9, 4, &[]), (0, cap - 3));
    assert_eq!(pool_insert(cap - 3, cap, 1, 4, &[]), (3, cap));
    assert_eq!(pool_insert(cap - 3, cap, 9, 4, &[Upgrade::Void]), (4, cap - 3));
    // Creative vending: unchanged pool.
    assert_eq!(pool_take(7, 1, 64, &[Upgrade::CreativeVending]), (64, 7));
    // Storage upgrades scale the pool.
    assert_eq!(pool_capacity(&ch, &[Upgrade::Storage(2)], &b, &c, 64), 64 * 32 * 4 * 81);
}

#[test]
fn network_search() {
    // A row of drawers x=1..=4 on the controller's east side, a gap, then one more.
    let mut world: HashMap<Pos, Node> = HashMap::new();
    for x in 1..=4 {
        world.insert([x, 0, 0], Node::Drawer);
    }
    world.insert([6, 0, 0], Node::Drawer);
    world.insert([2, 1, 0], Node::Other); // a trim above the second
    world.insert([2, 2, 0], Node::Drawer);
    let found = network([0, 0, 0], 50, |p| world.get(&p).copied().unwrap_or(Node::None));
    assert!(found.contains(&[1, 0, 0]) && found.contains(&[4, 0, 0]) && found.contains(&[2, 2, 0]));
    assert!(!found.contains(&[6, 0, 0]), "no adjacency, no connection");
    assert_eq!(found.len(), 5);
    // Range: the farthest drawers are cut off.
    let found = network([0, 0, 0], 2, |p| world.get(&p).copied().unwrap_or(Node::None));
    assert!(found.contains(&[2, 0, 0]) && !found.contains(&[4, 0, 0]));
    // Range 50 on a long line: exactly the first 50 blocks.
    let line = network([0, 0, 0], 50, |p| {
        if p[1] == 0 && p[2] == 0 && p[0] > 0 && p[0] < 100 { Node::Drawer } else { Node::None }
    });
    assert_eq!(line.len(), 50);
}

#[test]
fn keys_and_attributes() {
    let mut a = Attrs::default();
    assert!(Key::parse("storagedrawers:drawer_key").unwrap().apply(&mut a, "p"));
    assert!(a.locked);
    Key::parse("storagedrawers:keyring_drawer").unwrap().apply(&mut a, "p");
    assert!(!a.locked);
    Key::Quantify.apply(&mut a, "p");
    Key::Shroud.apply(&mut a, "p");
    Key::Suspend.apply(&mut a, "p");
    assert!(a.quantify && a.concealed && a.suspended);
    // Priority: the plain key cycles, the others set or clear.
    let mut p = Attrs::default();
    let seen: Vec<i32> = (0..6)
        .map(|_| {
            Key::PriorityCycle.apply(&mut p, "p");
            p.priority
        })
        .collect();
    assert_eq!(seen, vec![1, 2, -1, -2, 0, 1]);
    Key::Priority(2).apply(&mut p, "p");
    assert_eq!(p.priority, 2);
    Key::Priority(2).apply(&mut p, "p");
    assert_eq!(p.priority, 0);
    // Personal key: the first claims, the owner releases, others are refused.
    let mut o = Attrs::default();
    assert!(Key::Personal.apply(&mut o, "alice"));
    assert_eq!(o.owner.as_deref(), Some("alice"));
    assert!(!has_access(&o, "bob") && has_access(&o, "alice"));
    assert!(!Key::Personal.apply(&mut o, "bob"));
    assert!(Key::PersonalUnlock.apply(&mut o, "bob"));
    assert!(has_access(&o, "bob"));
    assert_eq!(Key::parse("storagedrawers:drawer_puller"), Some(Key::Puller));
    assert_eq!(Key::parse("storagedrawers:oak_trim"), None);
}

#[test]
fn the_world_file_round_trips() {
    let mut s = State::default();
    let mut a = Attrs {
        locked: true,
        priority: -2,
        owner: Some("0-0-0-1".into()),
        ..Attrs::default()
    };
    a.set_proto(1, Some("minecraft:cobblestone"));
    s.drawers.insert(("minecraft:overworld".into(), [1, 64, -5]), a.clone());
    s.drawers.insert(("minecraft:the_nether".into(), [-3, 10, 7]), Attrs { quantify: true, ..Attrs::default() });
    // A drawer with default attributes is not written.
    s.drawers.insert(("minecraft:overworld".into(), [9, 9, 9]), Attrs::default());
    let text = s.to_text();
    assert_eq!(text.lines().count(), 2);
    let back = State::from_text(&text);
    assert_eq!(back.drawers.len(), 2);
    assert_eq!(back.drawers[&("minecraft:overworld".to_string(), [1, 64, -5])], a);
    assert_eq!(back.drawers[&("minecraft:overworld".to_string(), [1, 64, -5])].proto(1), Some("minecraft:cobblestone"));
    assert_eq!(back.drawers[&("minecraft:overworld".to_string(), [1, 64, -5])].proto(0), None);
    // Garbage lines are skipped.
    assert_eq!(State::from_text("nonsense\n\nx\ty\n").drawers.len(), 0);
    // Trailing empty protos are trimmed.
    let mut t = Attrs::default();
    t.set_proto(2, Some("a:b"));
    t.set_proto(2, None);
    assert!(t.is_default());
}

#[test]
fn the_face_slots() {
    use Layout::*;
    // Border strip: nothing.
    assert_eq!(face_slot(Standard(1), 0.03, 0.5), None);
    assert_eq!(face_slot(Standard(1), 0.5, 0.95), None);
    assert_eq!(face_slot(Standard(1), 0.5, 0.5), Some(0));
    // 1x2: top is 0, bottom is 1.
    assert_eq!(face_slot(Standard(2), 0.5, 0.8), Some(0));
    assert_eq!(face_slot(Standard(2), 0.5, 0.2), Some(1));
    // 2x2: top-left 0, top-right 1, bottom-left 2, bottom-right 3.
    assert_eq!(face_slot(Standard(4), 0.2, 0.8), Some(0));
    assert_eq!(face_slot(Standard(4), 0.8, 0.8), Some(1));
    assert_eq!(face_slot(Standard(4), 0.2, 0.2), Some(2));
    assert_eq!(face_slot(Standard(4), 0.8, 0.2), Some(3));
    // Compacting: 2 tiers top / bottom; 3 tiers: top, bottom-left, bottom-right.
    assert_eq!(face_slot(Compacting(2), 0.7, 0.7), Some(0));
    assert_eq!(face_slot(Compacting(2), 0.7, 0.3), Some(1));
    assert_eq!(face_slot(Compacting(3), 0.7, 0.7), Some(0));
    assert_eq!(face_slot(Compacting(3), 0.2, 0.3), Some(1));
    assert_eq!(face_slot(Compacting(3), 0.8, 0.3), Some(2));
}

#[test]
fn the_shared_copy_is_the_rules() {
    // The game library's hooks (save ids and sizes) use a copy of the rules: it must be this file.
    assert_eq!(
        include_str!("../src/lib.rs"),
        include_str!("../mixins/storagedrawers/shared.rs")
    );
}

#[test]
fn the_world_crates_size_rule_matches() {
    // mixins/storagedrawers/pre_remove.rs cannot see the rules crate: the same rule, written out.
    fn spill(name: &str) -> Option<usize> {
        let path = name.strip_prefix("storagedrawers:")?;
        let at = path.rfind("_drawers_")?;
        let n: usize = path[at + 9..].parse().ok()?;
        let compacting = path.contains("compacting_");
        if !(compacting && (n == 2 || n == 3)) && !(!compacting && (n == 1 || n == 2 || n == 4)) {
            return None;
        }
        Some(n + 7)
    }
    let c = cfg();
    let mut names = vec!["storagedrawers:controller".to_string(), "storagedrawers:oak_trim".to_string(), "storagedrawers:framing_table".to_string(), "minecraft:chest".to_string()];
    for w in ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "bamboo", "crimson", "warped", "framed"] {
        for d in ["full", "half"] {
            for n in [1, 2, 3, 4, 5] {
                names.push(format!("storagedrawers:{w}_{d}_drawers_{n}"));
            }
        }
    }
    for f in ["", "framed_"] {
        for h in ["", "half_"] {
            for n in [1, 2, 3, 4] {
                names.push(format!("storagedrawers:{f}compacting_{h}drawers_{n}"));
            }
        }
    }
    for name in names {
        let rules_size = chest_size(&name, &c);
        let world_size = spill(&name);
        // The world-side rule may accept names the rules do not know (an unknown wood), never the reverse.
        if rules_size.is_some() {
            assert_eq!(rules_size, world_size, "{name}");
        }
    }
}
