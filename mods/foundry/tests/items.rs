// Item handler rules: ItemStackHandler slots, roles, stacking, moving without loss.
mod common;
use common::*;
use horizon_foundry::*;

fn view<'a>(inv: &'a mut Inventory<St>, roles: &'a [SlotRole], internal: bool) -> InvView<'a, St> {
    InvView { inv, roles, access: Access::Both, internal, max_stack: &max_stack, valid: None }
}

#[test]
fn insert_and_extract_like_item_stack_handler() {
    let mut inv = Inventory::new(2);
    let mut h = view(&mut inv, &[], false);
    assert_eq!(h.insert_item(0, st("minecraft:iron_ingot", 40), false), None);
    assert_eq!(h.insert_item(0, st("minecraft:iron_ingot", 40), false), Some(st("minecraft:iron_ingot", 16)));
    assert_eq!(h.insert_item(0, st("minecraft:gold_ingot", 1), false), Some(st("minecraft:gold_ingot", 1)), "other kind");
    assert_eq!(h.extract_item(0, 100, false), Some(st("minecraft:iron_ingot", 64)), "at most a stack");
    assert_eq!(h.stack_in_slot(0), None);
    assert_eq!(h.extract_item(1, 1, false), None);
}

#[test]
fn simulate_changes_nothing() {
    let mut inv = Inventory::new(1);
    let mut h = view(&mut inv, &[], false);
    assert_eq!(h.insert_item(0, st("minecraft:stone", 10), true), None);
    assert_eq!(h.stack_in_slot(0), None);
    h.insert_item(0, st("minecraft:stone", 10), false);
    assert_eq!(h.extract_item(0, 4, true), Some(st("minecraft:stone", 4)));
    assert_eq!(h.stack_in_slot(0), Some(st("minecraft:stone", 10)));
}

#[test]
fn stack_limits_and_components() {
    let mut inv = Inventory::new(1);
    let mut h = view(&mut inv, &[], false);
    assert_eq!(h.insert_item(0, st("minecraft:ender_pearl", 20), false), Some(st("minecraft:ender_pearl", 4)));
    let mut inv = Inventory::new(1);
    let mut h = view(&mut inv, &[], false);
    let mut named = st("minecraft:stone", 1);
    named.tag = Some("name".into());
    h.insert_item(0, st("minecraft:stone", 1), false);
    assert_eq!(h.insert_item(0, named.clone(), false), Some(named), "different components do not stack");
}

#[test]
fn roles_from_outside_and_inside() {
    let roles = [SlotRole::Input, SlotRole::Output, SlotRole::Internal];
    let mut inv = Inventory::new(3);
    {
        let mut out = view(&mut inv, &roles, false);
        assert_eq!(out.insert_item(0, st("minecraft:coal", 1), false), None);
        assert!(out.insert_item(1, st("minecraft:coal", 1), false).is_some(), "no insert into an output");
        assert!(out.insert_item(2, st("minecraft:coal", 1), false).is_some(), "no insert into an internal slot");
        assert_eq!(out.extract_item(0, 1, false), None, "no extract from an input");
    }
    {
        let mut inside = view(&mut inv, &roles, true);
        assert_eq!(inside.extract_item(0, 1, false), Some(st("minecraft:coal", 1)));
        assert_eq!(inside.insert_item(1, st("minecraft:iron_ingot", 2), false), None);
    }
    let mut out = view(&mut inv, &roles, false);
    assert_eq!(out.extract_item(1, 5, false), Some(st("minecraft:iron_ingot", 2)));
}

#[test]
fn validity_rule() {
    let mut inv = Inventory::new(1);
    let only_coal = |_: usize, s: &St| s.item == "minecraft:coal";
    let mut h = InvView {
        inv: &mut inv,
        roles: &[],
        access: Access::Both,
        internal: false,
        max_stack: &max_stack,
        valid: Some(&only_coal),
    };
    assert!(h.insert_item(0, st("minecraft:dirt", 1), false).is_some());
    assert_eq!(h.insert_item(0, st("minecraft:coal", 1), false), None);
}

#[test]
fn stacked_insert_fills_existing_stacks_first() {
    let mut inv = Inventory::new(3);
    inv.slots[2] = Some(st("minecraft:stone", 60));
    let mut h = view(&mut inv, &[], false);
    assert_eq!(insert_item_stacked(&mut h, st("minecraft:stone", 10), false), None);
    assert_eq!(inv.slots[2], Some(st("minecraft:stone", 64)));
    assert_eq!(inv.slots[0], Some(st("minecraft:stone", 6)));
    let mut inv = Inventory::new(3);
    inv.slots[2] = Some(st("minecraft:stone", 60));
    let mut h = view(&mut inv, &[], false);
    assert_eq!(insert_item(&mut h, st("minecraft:stone", 10), false), None);
    assert_eq!(inv.slots[0], Some(st("minecraft:stone", 10)), "insertItem goes slot by slot");
}

#[test]
fn moving_never_loses_or_makes_items() {
    let mut rng = Rng(77);
    let items = ["minecraft:stone", "minecraft:ender_pearl", "minecraft:iron_pickaxe", "minecraft:coal"];
    let mut a = Inventory::new(9);
    let mut b = Inventory::new(4);
    for i in 0..9 {
        let it = items[i % 4];
        a.slots[i] = Some(st(it, 1 + (i as u32 * 7) % max_stack(&st(it, 1))));
    }
    let count = |a: &Inventory<St>, b: &Inventory<St>, it: &str| {
        a.slots.iter().chain(b.slots.iter()).flatten().filter(|s| s.item == it).map(|s| s.count).sum::<u32>()
    };
    let before: Vec<u32> = items.iter().map(|it| count(&a, &b, it)).collect();
    for _ in 0..300 {
        let max = rng.next(80) as u32;
        let (n, stray) = if rng.next(2) == 0 {
            move_items(&mut view(&mut a, &[], false), &mut view(&mut b, &[], false), max)
        } else {
            move_items(&mut view(&mut b, &[], false), &mut view(&mut a, &[], false), max)
        };
        assert!(n <= max);
        assert!(stray.is_empty());
    }
    let after: Vec<u32> = items.iter().map(|it| count(&a, &b, it)).collect();
    assert_eq!(before, after);
}
