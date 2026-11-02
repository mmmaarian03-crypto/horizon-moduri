use horizon_soulshards::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Map(BTreeMap<Pos, String>);
impl World for Map {
    fn block(&self, p: Pos) -> Option<String> {
        Some(self.0.get(&p).cloned().unwrap_or_else(|| "minecraft:air".into()))
    }
}

fn shard(b: Option<Binding>) -> Option<ShardView> {
    Some(ShardView { is_shard: true, binding: b })
}
fn bound(e: &str, kills: i32) -> Binding {
    Binding { bound: Some(e.into()), owner: Some([1, 2, 3, 4]), kills }
}

#[test]
fn tiers_follow_tuned_tiers_json() {
    assert_eq!(max_kills(), 1024);
    for (kills, t) in [(0, 0), (63, 0), (64, 1), (127, 1), (128, 2), (255, 2), (256, 3), (511, 3), (512, 4), (1023, 4), (1024, 5)] {
        assert_eq!(tier_index(kills), t, "{kills} kills");
    }
    assert_eq!(tier(64).spawn_amount, 2);
    assert_eq!(tier(64).cooldown, 400);
    assert_eq!(tier(1024).cooldown, 40);
    assert!(tier(1024).check_redstone && !tier(1024).check_player);
    // The jar's Tier.generateDefaults equals the file.
    use horizon_soulshards::date_jar::*;
    let t = |x: (i32, bool, bool, bool, i32, i32)| Tier {
        kill_requirement: x.0,
        check_player: x.1,
        check_light: x.2,
        check_redstone: x.3,
        spawn_amount: x.4,
        cooldown: x.5,
    };
    assert_eq!(TIERS, [t(TIER_TIER), t(TIER_TIER_2), t(TIER_TIER_3), t(TIER_TIER_4), t(TIER_TIER_5), t(TIER_TIER_6)]);
}

#[test]
fn kills_are_capped_at_the_last_tier() {
    let mut b = bound("minecraft:zombie", 1000);
    b.add_kills(200);
    assert_eq!(b.kills, 1024);
    assert!(b.is_full());
    assert_eq!(model_name(&Some(b)), Some("tier_5".into()));
    assert_eq!(model_name(&None), None);
}

#[test]
fn first_shard_checks_the_offhand_then_the_hotbar() {
    let mut hot: [Option<ShardView>; 9] = Default::default();
    hot[3] = shard(Some(bound("minecraft:skeleton", 5)));
    hot[5] = shard(None);
    // A wrong offhand shard means no shard (the hotbar is not searched).
    assert_eq!(first_shard(&shard(Some(bound("minecraft:skeleton", 5))), &hot, "minecraft:zombie"), None);
    // No shard in the offhand: the first fitting hotbar shard.
    assert_eq!(first_shard(&None, &hot, "minecraft:zombie"), Some(ShardSlot::Hotbar(5)));
    assert_eq!(first_shard(&None, &hot, "minecraft:skeleton"), Some(ShardSlot::Hotbar(3)));
    let not_shard = Some(ShardView { is_shard: false, binding: None });
    assert_eq!(first_shard(&not_shard, &hot, "minecraft:skeleton"), Some(ShardSlot::Hotbar(3)));
    // Full shards take no more souls.
    hot[3] = shard(Some(bound("minecraft:skeleton", 1024)));
    hot[5] = None;
    assert_eq!(first_shard(&None, &hot, "minecraft:skeleton"), None);
    // A binding without entity takes anything.
    assert_eq!(
        first_shard(&shard(Some(Binding { bound: None, owner: None, kills: 3 })), &hot, "minecraft:cow"),
        Some(ShardSlot::OffHand)
    );
}

#[test]
fn which_kills_count() {
    let k = |by_player, allowed, boss, cage_born| Kill { entity: "minecraft:zombie".into(), by_player, allowed, boss, cage_born };
    assert!(kill_counts(&k(true, true, false, false)));
    assert!(!kill_counts(&k(false, true, false, false)), "not killed by a player");
    assert!(!kill_counts(&k(true, false, false, false)), "#soulshards:no_capture");
    assert!(!kill_counts(&k(true, true, true, false)), "allowBossSpawns = false");
    assert!(!kill_counts(&k(true, true, false, true)), "countCageBornForShard = false");
}

#[test]
fn souls_per_kill_and_binding() {
    assert_eq!(souls_for_kill(0, false), 1);
    assert_eq!(souls_for_kill(5, false), 6);
    assert_eq!(souls_for_kill(2, true), 4);
    let b = bind_kill(None, "minecraft:zombie", [9, 9, 9, 9], 3);
    assert_eq!(b, Binding { bound: Some("minecraft:zombie".into()), owner: Some([9, 9, 9, 9]), kills: 3 });
    // The first owner and entity stay.
    let b2 = bind_kill(Some(b), "minecraft:zombie", [1, 1, 1, 1], 2);
    assert_eq!(b2.owner, Some([9, 9, 9, 9]));
    assert_eq!(b2.kills, 5);
}

#[test]
fn anvil_combines_shards_of_one_entity() {
    let (out, cost) = combine(&bound("minecraft:zombie", 100), &bound("minecraft:zombie", 50)).unwrap();
    assert_eq!(out.kills, 150);
    assert_eq!(cost, 6); // tier 1 * 6
    assert!(combine(&bound("minecraft:zombie", 1), &bound("minecraft:skeleton", 1)).is_none());
}

#[test]
fn the_altar_is_the_3x3_of_the_jar() {
    let mut w = Map::default();
    let o = [10, 64, -5];
    for (dz, line) in SHAPE.iter().enumerate() {
        for (dx, c) in line.chars().enumerate() {
            w.0.insert([o[0] + dx as i32 - 1, 64, o[2] + dz as i32 - 1], slot_block(c).into());
        }
    }
    assert!(is_origin("minecraft:glowstone"));
    let m = multiblock_match(&w, o).unwrap();
    assert_eq!(m.len(), 9);
    assert!(m.contains(&[9, 64, -6]) && m.contains(&[11, 64, -4]));
    // Corners must be obsidian, edges quartz blocks.
    w.0.insert([9, 64, -6], "minecraft:quartz_block".into());
    assert!(multiblock_match(&w, o).is_none());
    w.0.insert([9, 64, -6], "minecraft:obsidian".into());
    w.0.insert([10, 64, -6], "minecraft:smooth_quartz".into());
    assert!(multiblock_match(&w, o).is_none(), "1.3.6 takes only quartz_block");
}

fn check(kills: i32) -> CageCheck {
    CageCheck { binding: Some(bound("minecraft:zombie", kills)), allowed: true, powered: false, player_near: true, owner_online: true }
}

#[test]
fn cage_spawn_conditions() {
    assert!(can_spawn(&check(10)).is_none(), "tier 0 spawns nothing");
    assert!(can_spawn(&check(64)).is_some());
    let mut c = check(64);
    c.player_near = false;
    assert!(can_spawn(&c).is_none(), "tier 1 needs a player");
    let mut c = check(256);
    c.player_near = false;
    assert!(can_spawn(&c).is_some(), "tier 3 does not");
    let mut c = check(1024);
    c.powered = true;
    assert!(can_spawn(&c).is_none(), "tier 5 stops with redstone");
    let mut c = check(512);
    c.powered = true;
    assert!(can_spawn(&c).is_some(), "only tier 5 checks redstone");
    let mut c = check(512);
    c.allowed = false;
    assert!(can_spawn(&c).is_none());
    let mut c = check(512);
    c.binding.as_mut().unwrap().bound = None;
    assert!(can_spawn(&c).is_none());
}

#[test]
fn cage_ticks_spawn_every_cooldown_and_switch_the_block() {
    let mut cage = Cage { shard: Some("x".into()), active: false, active_time: 0 };
    let c = check(64); // cooldown 400
    let first = cage_tick(&mut cage, &c);
    assert_eq!(first.set_active, Some(true));
    assert!(!first.spawn);
    let mut spawns = 0;
    for _ in 1..800 {
        let t = cage_tick(&mut cage, &c);
        assert_eq!(t.set_active, None);
        spawns += usize::from(t.spawn);
    }
    assert_eq!(cage.active_time, 800);
    assert_eq!(spawns, 2, "at activeTime 400 and 800");
    let mut off = c.clone();
    off.player_near = false;
    let t = cage_tick(&mut cage, &off);
    assert_eq!(t.set_active, Some(false));
    assert!(!cage.active);
    assert_eq!(cage_tick(&mut cage, &off).set_active, None);
}

#[test]
fn spawn_positions_truncate_like_java() {
    // x = 10 + (0.9 - 0.1) * 4 + 0.5 = 13.7 -> 13; z = -10 + (0.0 - 0.5) * 4 + 0.5 = -11.5 -> -11 (towards zero).
    assert_eq!(spawn_pos([10, 64, -10], 0.9, 0.1, 2, 0.0, 0.5), [13, 66, -11]);
    // The cage itself: one block up.
    assert_eq!(spawn_pos([10, 64, 10], 0.5, 0.5, 0, 0.5, 0.5), [10, 65, 10]);
    assert!(light_ok(8) && !light_ok(9));
}

#[test]
fn cage_state_round_trips() {
    let mut s = State::default();
    s.cages.insert([1, -2, 3], Cage { shard: Some("0a0b".into()), active: true, active_time: 41 });
    s.cages.insert([5, 6, 7], Cage { shard: None, active: false, active_time: 0 });
    assert_eq!(State::load(Some(s.save())), s);
    assert_eq!(State::load(Some("garbage\n1 2\n".into())), State::default());
}

#[test]
fn tooltip_lines() {
    let b = bound("minecraft:zombie", 70);
    let lines = tooltip(&b, true, false, uuid_text);
    assert_eq!(
        lines,
        vec![
            ("tooltip.soulshards.bound", "minecraft:zombie".to_string()),
            ("tooltip.soulshards.tier", "1".to_string()),
            ("tooltip.soulshards.kills", "70".to_string()),
        ]
    );
    assert_eq!(tooltip(&b, true, true, uuid_text).len(), 4);
    assert_eq!(fill("Bound to: %s", &["minecraft:zombie".into()]), "Bound to: minecraft:zombie");
    assert_eq!(fill("Souls: %d (100%%)", &["5".into()]), "Souls: 5 (100%)");
    assert_eq!(uuid_text([0x01234567, 0x89abcdef_u32 as i32, 0x01234567, 0x89abcdef_u32 as i32]), "01234567-89ab-cdef-0123-456789abcdef");
}
