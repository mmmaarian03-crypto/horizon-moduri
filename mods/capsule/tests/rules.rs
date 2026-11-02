use horizon_capsule::*;
use std::collections::BTreeMap;

fn capsule(state: State, size: i32) -> Capsule {
    Capsule { state: state.value(), size, ..Default::default() }
}

#[test]
fn sizes_are_odd_and_capped() {
    assert_eq!(capsule(State::Empty, 0).size(), 1);
    assert_eq!(capsule(State::Empty, 4).size(), 5);
    assert_eq!(capsule(State::Empty, 300).size(), 255);
    assert_eq!(capsule(State::Empty, 7).extend(), 3);
    for v in 0..8 {
        assert_eq!(State::from_value(v).unwrap().value(), v);
    }
    assert_eq!(State::from_value(9), None);
    assert_eq!(capsule(State::Linked, 3).state().model(), "linked");
}

#[test]
fn right_click_follows_the_capsule_state() {
    let now = 1000;
    assert_eq!(use_action(&capsule(State::Empty, 3), true, false, now), UseAction::Activate(State::EmptyActivated));
    assert_eq!(use_action(&capsule(State::Empty, 3), false, false, now), UseAction::Nothing, "offhand fails");
    let mut linked = capsule(State::Linked, 3);
    linked.structure = Some("c-x-0".into());
    assert_eq!(use_action(&linked, true, false, now), UseAction::Activate(State::Activated));
    assert_eq!(use_action(&linked, true, true, now), UseAction::Label);
    assert_eq!(use_action(&capsule(State::EmptyActivated, 3), true, false, now), UseAction::Throw);
    let mut deployed = capsule(State::Deployed, 3);
    assert_eq!(use_action(&deployed, true, false, now), UseAction::Nothing, "no spawn position");
    deployed.spawn = Some(([0, 64, 0], "minecraft:overworld".into()));
    assert_eq!(use_action(&deployed, true, false, now), UseAction::Undeploy);
    // Size 1: instant, both empty and linked; a deployed size-1 capsule undeploys normally.
    assert_eq!(use_action(&capsule(State::Empty, 1), true, false, now), UseAction::Instant);
    assert_eq!(use_action(&capsule(State::Linked, 1), true, false, now), UseAction::Instant);
    let mut d1 = capsule(State::Deployed, 1);
    d1.spawn = deployed.spawn.clone();
    assert_eq!(use_action(&d1, true, false, now), UseAction::Undeploy);
    // The click that deployed an instant capsule does not undeploy it (5 ticks).
    d1.undeploy_at = now + 5;
    assert_eq!(use_action(&d1, true, false, now), UseAction::Nothing);
    assert_eq!(use_action(&d1, true, false, now + 5), UseAction::Undeploy);
    // A date from another world (further than the delay) is ignored.
    d1.undeploy_at = now + 1000;
    assert_eq!(use_action(&d1, true, false, now), UseAction::Undeploy);
}

#[test]
fn activation_times_out_after_120_ticks() {
    let mut c = capsule(State::Activated, 3);
    c.structure = Some("c-x-1".into());
    c.timer_start = Some(100);
    assert_eq!(c.timer_tick(219), None);
    assert_eq!(c.timer_tick(220), Some(true));
    assert!(c.has(State::Linked));
    assert_eq!(c.timer_start, None);
    let mut e = capsule(State::EmptyActivated, 3);
    assert_eq!(e.timer_tick(5), Some(false), "no timer: back at once");
    assert!(e.has(State::Empty));
    let mut o = capsule(State::OneUseActivated, 3);
    o.one_use = true;
    o.timer_start = Some(500);
    assert_eq!(o.timer_tick(10), Some(true), "a start in the future (another world)");
    assert!(o.has(State::OneUse));
    assert_eq!(capsule(State::Linked, 3).timer_tick(10_000), None);
}

#[test]
fn block_pos_longs_match_java() {
    for p in [[0, 0, 0], [1, -64, 1], [-30_000_000, 319, 29_999_999], [123, 64, -456]] {
        assert_eq!(pos_of_long(pos_to_long(p)), p);
    }
    // BlockPos.asLong(1, 2, 3) = 1 << 38 | 3 << 12 | 2.
    assert_eq!(pos_to_long([1, 2, 3]), (1i64 << 38) | (3 << 12) | 2);
}

#[test]
fn landing_and_anchors() {
    // An item lying on the ground at y 64.0 above block 63: the anchor is the ground block under it.
    assert_eq!(find_bottom_block(10.3, 64.0, -5.7), [10, 63, -6]);
    // Capture base facing up (dispenser facing): the area starts on top of it.
    assert_eq!(anchor([0, 64, 0], [0, 1, 0], 3), [0, 64, 0]);
    assert_eq!(capture_source(anchor([0, 64, 0], [0, 1, 0], 3), 1), [-1, 65, -1]);
    // Facing down: the area hangs below it.
    assert_eq!(anchor([0, 64, 0], [0, -1, 0], 3), [0, 60, 0]);
    // Facing east: half the size + 0.5 to the side, one down.
    assert_eq!(anchor([0, 64, 0], [1, 0, 0], 3), [2, 63, 0]);
    assert_eq!(deploy_dest([5, 63, 5], 2, None), [3, 64, 3]);
    assert_eq!(deploy_dest([5, 63, 5], 2, Some([10, 70, 10])), [8, 70, 8]);
    assert_eq!(deploy_target([4, 63, 4], [0, 1, 0], -2), [4, 62, 4]);
    // The capture base search: range 1 first.
    let first: Vec<Pos> = search_order(0.5, 64.0, 0.5, 3).take(3).collect();
    assert_eq!(first, vec![[-1, 63, -1], [0, 63, -1], [1, 63, -1]]);
}

#[test]
fn exclusions_follow_tuned_lists() {
    assert!(excluded("minecraft:bedrock", false, false));
    assert!(excluded("minecraft:spawner", false, false));
    assert!(!excluded("minecraft:spawner", true, false), "overpowered capsules take spawners");
    assert!(excluded("refinedstorage:controller", false, false), "mod prefix");
    assert!(excluded("mekanism:boundingblock", true, false));
    assert!(!excluded("mekanism:steel_casing", false, false));
    assert!(excluded("minecraft:stone", false, true), "#capsule:excluded");
    assert!(!excluded("minecraft:stone", false, false));
}

fn world(blocks: &[(Pos, &str)]) -> BTreeMap<Pos, String> {
    blocks.iter().map(|(p, b)| (*p, b.to_string())).collect()
}

#[test]
fn a_capture_takes_everything_but_air_excluded_and_what_was_there() {
    let w = world(&[([0, 0, 0], "minecraft:stone"), ([1, 0, 0], "minecraft:bedrock"), ([2, 2, 2], "minecraft:chest"), ([0, 1, 0], "minecraft:dirt")]);
    let at = |p: Pos| w.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into());
    let air = |b: &str| b == "minecraft:air";
    let ex = |b: &str| excluded(b, false, false);
    let got = capture_positions([0, 0, 0], 3, at, air, ex, None);
    assert_eq!(got, vec![[0, 0, 0], [0, 1, 0], [2, 2, 2]], "x, then y, then z");
    // Undeploy: the dirt was there before the deploy (same block): it stays.
    let occupied = world(&[([0, 1, 0], "minecraft:dirt"), ([0, 0, 0], "minecraft:grass_block")]);
    let got = capture_positions([0, 0, 0], 3, at, air, ex, Some(&occupied));
    assert_eq!(got, vec![[0, 0, 0], [2, 2, 2]], "the stone replaced the grass: taken");
}

#[test]
fn deploy_checks_the_destination() {
    let air = |b: &str| b == "minecraft:air";
    let over = |b: &str| matches!(b, "minecraft:short_grass" | "minecraft:snow" | "minecraft:oak_leaves");
    let template = world(&[([10, 64, 10], "minecraft:stone")]);
    let tat = |p: Pos| template.get(&p).cloned();
    let none = |_: Pos| None;
    // Grass is overridable: fine, and recorded as occupied.
    let w = world(&[([11, 64, 11], "minecraft:short_grass")]);
    let wat = |p: Pos| Some(w.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()));
    let occupied = check_destination([10, 64, 10], 3, tat, wat, air, over, none).unwrap();
    assert_eq!(occupied.len(), 1);
    assert!(deploy_places(&occupied, [11, 64, 11], over));
    // A stone anywhere in the area blocks, even where the template is empty.
    let w = world(&[([12, 66, 12], "minecraft:dirt")]);
    let wat = |p: Pos| Some(w.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()));
    assert_eq!(check_destination([10, 64, 10], 3, tat, wat, air, over, none), Err(DeployError::Block([12, 66, 12])));
    // A mob where a template block goes.
    let empty = |_: Pos| Some("minecraft:air".to_string());
    let cow = |p: Pos| (p == [10, 64, 10]).then(|| "Cow".to_string());
    assert_eq!(check_destination([10, 64, 10], 3, tat, empty, air, over, cow), Err(DeployError::Entity("Cow".into())));
    // ...but not where the template is air.
    let cow2 = |p: Pos| (p == [11, 64, 10]).then(|| "Cow".to_string());
    assert!(check_destination([10, 64, 10], 3, tat, empty, air, over, cow2).is_ok());
    assert_eq!(check_destination([10, 64, 10], 3, tat, |_| None, air, over, none), Err(DeployError::NotLoaded));
    // Occupied by a non-overridable block: the template block is not placed there (merge).
    let occ = world(&[([10, 64, 10], "minecraft:dirt")]);
    assert!(!deploy_places(&occ, [10, 64, 10], over));
}

#[test]
fn throws_aim_at_the_target() {
    let v = throw_velocity([0, 64, 0], Some([10, 64, 0]), [0.5, 65.3, 0.5], 0.0, 0.0, [0.0; 3]);
    assert!(v[0] > 0.9 && v[0] <= 1.2 + 1e-9 && v[2].abs() < 1e-9, "{v:?}");
    // dy = 0 + min(1, 10.5/3) = 1 -> 1/10 + 0.5 * 10 * 0.04 = 0.3.
    assert!((v[1] - 0.3).abs() < 1e-6, "{v:?}");
    // Blind throw facing south (yaw 0): +z.
    let b = throw_velocity([0, 64, 0], None, [0.0; 3], 0.0, 0.0, [0.0; 3]);
    assert!(b[2] > 0.49 && b[0].abs() < 1e-6 && (b[1] - 0.1).abs() < 1e-6, "{b:?}");
    let (vx, vz) = move_toward([0.5, 64.0, 0.5], [1.0, 0.0, 0.0], [20, 64, 0], true);
    assert!((vx - (0.9 + 0.1 * 1.2)).abs() < 1e-6 && vz.abs() < 1e-9);
}

#[test]
fn names_labels_and_counters() {
    assert_eq!(unique_name("Tuned_V", 3), "c-tuned_v-3");
    assert_eq!(unique_name("Ab Cd!", 0), "c-abcd-0");
    assert_eq!(label_from_path("config/capsule/starters/_stater_oak_hut"), " Stater Oak Hut");
    assert_eq!(label_from_path("old_house"), "Old House");
    let mut s = State2::default();
    assert_eq!(s.next(), 0);
    assert_eq!(s.next(), 1);
    s.starters.push("u".into());
    assert_eq!(State2::load(Some(s.save())), s);
    assert_eq!(size_of_template([7, 5, 6]), 7);
    assert_eq!(size_of_template([8, 5, 6]), 9);
}

#[test]
fn item_names_follow_get_name() {
    let empty = capsule(State::Empty, 3);
    assert_eq!(name_parts(&empty), (None, Some(Content::Empty), "items.capsule.name"));
    let mut linked = capsule(State::Linked, 3);
    linked.structure = Some("c-a-1".into());
    assert_eq!(name_parts(&linked), (None, Some(Content::Unlabeled), "items.capsule.name"));
    linked.label = Some("Hut".into());
    assert_eq!(name_parts(&linked).1, Some(Content::Label("Hut".into())));
    let mut d = linked.clone();
    d.state = State::Deployed.value();
    assert_eq!(name_parts(&d).0, Some("items.capsule.state_deployed"));
    let mut a = linked.clone();
    a.state = State::Activated.value();
    assert_eq!(name_parts(&a).0, Some("items.capsule.state_activated"));
    let mut o = linked.clone();
    o.state = State::OneUse.value();
    o.one_use = true;
    assert_eq!(name_parts(&o).0, Some("items.capsule.state_recovery"));
    o.is_reward = true;
    assert_eq!(name_parts(&o).0, Some("items.capsule.state_one_use"));
    let t = tooltip(&o);
    assert!(t.contains(&Line::Key("capsule.tooltip.one_use")));
    assert!(t.contains(&Line::Size { size: 3, upgraded: 0, instant: false }));
}
