use horizon_carryon::*;

fn no_tag(_: &str) -> bool {
    false
}

#[test]
fn wildcards_follow_string_helper() {
    // "minecraft:*_bed": the pieces "minecraft:" and "_bed" in order.
    assert!(list_matches(&["minecraft:*_bed"], "minecraft:red_bed", &no_tag));
    assert!(!list_matches(&["minecraft:*_bed"], "minecraft:bedrock", &no_tag));
    // A trailing star: "ic2:*" = "ic2:" contained.
    assert!(list_matches(&["ic2:*"], "ic2:generator", &no_tag));
    assert!(!list_matches(&["ic2:*"], "minecraft:chest", &no_tag));
    // Exact names, no stars.
    assert!(list_matches(&["minecraft:cake"], "minecraft:cake", &no_tag));
    assert!(!list_matches(&["minecraft:cake"], "minecraft:candle_cake", &no_tag));
    // Tags go through the lookup.
    assert!(list_matches(&["#forge:immovable"], "x:y", &|t| t == "forge:immovable"));
    // Pieces must come in order.
    assert!(!list_matches(&["a:*z*b"], "a:bz", &no_tag));
    assert!(list_matches(&["thaumcraft:pillar*"], "thaumcraft:pillar_arcane", &no_tag));
}

#[test]
fn tuned_blacklists() {
    for b in ["minecraft:red_bed", "minecraft:oak_door", "minecraft:iron_door", "minecraft:tall_grass", "minecraft:cake", "create:belt", "refinedstorage:grid", "powah:energy_cell", "minecraft:end_portal"] {
        assert!(!block_permitted(b, &no_tag), "{b}");
    }
    for b in ["minecraft:chest", "minecraft:furnace", "minecraft:barrel", "minecraft:spawner", "ironchest:gold_chest", "storagedrawers:oak_full_drawers_1"] {
        assert!(block_permitted(b, &no_tag), "{b}");
    }
    for e in ["minecraft:ghast", "minecraft:armor_stand", "minecraft:item_frame", "minecraft:painting", "minecraft:ender_dragon", "pixelmon:pikachu", "bodiesbodies:dead_body"] {
        assert!(!entity_permitted(e, &no_tag), "{e}");
    }
    for e in ["minecraft:pig", "minecraft:villager", "minecraft:cow", "minecraft:zombie"] {
        assert!(entity_permitted(e, &no_tag), "{e}");
    }
    // Only horses cannot hold a pile.
    assert!(!stacking_permitted("minecraft:horse", &no_tag));
    assert!(stacking_permitted("minecraft:cow", &no_tag));
}

#[test]
fn tuned_values() {
    assert_eq!(config::SETTINGS_MAX_DISTANCE, 2.5);
    assert_eq!((config::SETTINGS_MAX_ENTITY_WIDTH, config::SETTINGS_MAX_ENTITY_HEIGHT), (1.5, 2.5));
    assert_eq!(config::SETTINGS_MAX_ENTITY_STACK_LIMIT, 10);
    assert!(config::SETTINGS_HEAVY_TILES && config::SETTINGS_HEAVY_ENTITIES && config::SETTINGS_STACKABLE_ENTITIES);
    assert!(!config::SETTINGS_PICKUP_ALL_BLOCKS && !config::SETTINGS_PICKUP_HOSTILE_MOBS && !config::SETTINGS_ALLOW_BABIES);
    assert!(!config::SETTINGS_HIT_WHILE_CARRYING && !config::SETTINGS_DROP_CARRIED_WHEN_HIT);
    assert!(config::SETTINGS_PICKUP_PLAYERS && !config::SETTINGS_PICKUP_UNBREAKABLE_BLOCKS);
    assert!(config::SETTINGS_SLOWNESS_IN_CREATIVE);
}

fn carrier() -> Carrier {
    Carrier { hands_empty: true, distance: 2.0, carrying: false, key_pressed: true, same_tick: false, game_mode_allows: true, creative: false }
}

#[test]
fn general_pickup_rules() {
    assert_eq!(can_carry_general(&carrier()), Ok(()));
    assert_eq!(can_carry_general(&Carrier { hands_empty: false, ..carrier() }), Err(Deny::Hands));
    assert_eq!(can_carry_general(&Carrier { distance: 2.6, ..carrier() }), Err(Deny::Distance));
    assert_eq!(can_carry_general(&Carrier { distance: 2.5, ..carrier() }), Ok(()));
    assert_eq!(can_carry_general(&Carrier { carrying: true, ..carrier() }), Err(Deny::AlreadyCarrying));
    assert_eq!(can_carry_general(&Carrier { key_pressed: false, ..carrier() }), Err(Deny::Key));
    assert_eq!(can_carry_general(&Carrier { same_tick: true, ..carrier() }), Err(Deny::SameTick));
    assert_eq!(can_carry_general(&Carrier { game_mode_allows: false, ..carrier() }), Err(Deny::GameMode));
}

#[test]
fn blocks_that_can_be_picked_up() {
    let chest = BlockFacts { name: "minecraft:chest".into(), has_block_entity: true, ..Default::default() };
    assert_eq!(can_pick_block(&chest, false, &no_tag), Ok(()));
    // Not a tile entity: only with pickupAllBlocks (off).
    let stone = BlockFacts { name: "minecraft:stone".into(), ..Default::default() };
    assert_eq!(can_pick_block(&stone, false, &no_tag), Err(Deny::NotATile));
    let bed = BlockFacts { name: "minecraft:red_bed".into(), has_block_entity: true, ..Default::default() };
    assert_eq!(can_pick_block(&bed, true, &no_tag), Err(Deny::Forbidden));
    let unbreakable = BlockFacts { name: "minecraft:spawner".into(), has_block_entity: true, unbreakable: true, ..Default::default() };
    assert_eq!(can_pick_block(&unbreakable, false, &no_tag), Err(Deny::Unbreakable));
    assert_eq!(can_pick_block(&unbreakable, true, &no_tag), Ok(()), "creative can");
    let locked = BlockFacts { name: "minecraft:chest".into(), has_block_entity: true, locked: true, ..Default::default() };
    assert_eq!(can_pick_block(&locked, false, &no_tag), Err(Deny::Locked));
}

fn pig() -> EntityFacts {
    EntityFacts { kind: "minecraft:pig".into(), width: 0.9, height: 0.9, ..Default::default() }
}

#[test]
fn entities_that_can_be_picked_up() {
    assert_eq!(can_pick_entity(&pig(), false, &no_tag), Ok(()));
    let zombie = EntityFacts { kind: "minecraft:zombie".into(), is_monster: true, width: 0.6, height: 1.95, ..Default::default() };
    assert_eq!(can_pick_entity(&zombie, false, &no_tag), Err(Deny::Hostile));
    assert_eq!(can_pick_entity(&zombie, true, &no_tag), Ok(()), "creative takes hostile mobs");
    // Iron golem: 1.4 x 2.7 - taller than 2.5.
    assert_eq!(can_pick_entity(&EntityFacts { kind: "minecraft:iron_golem".into(), width: 1.4, height: 2.7, ..Default::default() }, false, &no_tag), Err(Deny::TooBig));
    // A horse 1.3964844 x 1.6 fits.
    assert_eq!(can_pick_entity(&EntityFacts { kind: "minecraft:horse".into(), width: 1.3964844, height: 1.6, ..Default::default() }, false, &no_tag), Ok(()));
    assert_eq!(can_pick_entity(&EntityFacts { kind: "minecraft:ghast".into(), width: 4.0, height: 4.0, ..Default::default() }, true, &no_tag), Err(Deny::Forbidden));
    // A baby of a forbidden mob: only with allowBabies (off).
    assert_eq!(can_pick_entity(&EntityFacts { kind: "minecraft:ghast".into(), is_baby: true, width: 1.0, height: 1.0, ..Default::default() }, false, &no_tag), Err(Deny::Forbidden));
    assert_eq!(can_pick_entity(&EntityFacts { owned_by_other: true, ..pig() }, false, &no_tag), Err(Deny::OtherOwner));
    assert_eq!(can_pick_entity(&EntityFacts { invulnerable_time: true, ..pig() }, false, &no_tag), Err(Deny::Invulnerable));
    assert_eq!(can_pick_entity(&EntityFacts { removed: true, ..pig() }, false, &no_tag), Err(Deny::Removed));
    // Players: creative players cannot be picked up by a survival player.
    let other = EntityFacts { kind: "minecraft:player".into(), is_player: true, other_player_creative: true, width: 0.6, height: 1.8, ..Default::default() };
    assert_eq!(can_pick_entity(&other, false, &no_tag), Err(Deny::Players));
    assert_eq!(can_pick_entity(&EntityFacts { other_player_creative: false, ..other }, false, &no_tag), Ok(()));
}

#[test]
fn slowness_levels() {
    assert_eq!(slowness_amplifier(Load::Player), 1);
    // Entities: floor(width * height), at most 4. A pig 0.81 -> 0, a horse 2.2 -> 2.
    assert_eq!(slowness_amplifier(Load::Entity { area: 0.9 * 0.9 }), 0);
    assert_eq!(slowness_amplifier(Load::Entity { area: 1.3964844 * 1.6 }), 2);
    assert_eq!(slowness_amplifier(Load::Entity { area: 100.0 }), 4);
    // Blocks: the data text length / 500, at most 4.
    assert_eq!(slowness_amplifier(Load::Block { nbt_len: 100 }), 0);
    assert_eq!(slowness_amplifier(Load::Block { nbt_len: 1499 }), 2);
    assert_eq!(slowness_amplifier(Load::Block { nbt_len: 50_000 }), 4);
    assert!(slows(true) && slows(false));
    assert!(!may_hit(true) && may_hit(false));
}

fn p(a: &[(&str, &str)]) -> Vec<(String, String)> {
    a.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn placement_state_keeps_the_carried_state() {
    let ex = property_exceptions();
    assert_eq!(ex.len(), 7);
    // A furnace facing north and lit, placed by a player looking east: facing follows the player, lit stays.
    let carried = p(&[("facing", "north"), ("lit", "true")]);
    let placed = p(&[("facing", "east"), ("lit", "false")]);
    assert_eq!(merge_placement_state(&carried, &placed, &ex), p(&[("facing", "east"), ("lit", "true")]));
    // A chest: type follows the neighbours (exception), waterlogged the water.
    let carried = p(&[("facing", "south"), ("type", "single"), ("waterlogged", "false")]);
    let placed = p(&[("facing", "west"), ("type", "left"), ("waterlogged", "true")]);
    assert_eq!(merge_placement_state(&carried, &placed, &ex), p(&[("facing", "west"), ("type", "left"), ("waterlogged", "true")]));
    // Axis of a log-like block; a property the placed state does not have is left.
    let carried = p(&[("axis", "y"), ("extra", "1")]);
    assert_eq!(merge_placement_state(&carried, &p(&[("axis", "x")]), &ex), p(&[("axis", "x"), ("extra", "1")]));
    // A vine side is an exception (boolean), a bed part is not.
    assert!(is_property_exception("north", "true", &ex));
    assert!(!is_property_exception("north", "none", &ex));
    assert!(is_property_exception("rotation", "7", &ex));
    assert!(is_property_exception("face", "wall", &ex));
    assert!(!is_property_exception("part", "head", &ex));
    assert!(!is_property_exception("type", "top", &ex), "the type of a slab is not the chest type");
    assert!(is_property_exception("half", "top", &ex));
}

#[test]
fn placement_position_and_death_spot() {
    assert_eq!(placement_pos([5, 64, 5], [0, 1, 0], true), [5, 64, 5]);
    assert_eq!(placement_pos([5, 64, 5], [0, 1, 0], false), [5, 65, 5]);
    assert_eq!(placement_pos([5, 64, 5], [-1, 0, 0], false), [4, 64, 5]);
    let c = death_candidates([0, 64, 0], [0.5, 64.0, 0.5]);
    assert_eq!(c.len(), 30 * 30 * 30);
    // The nearest to the center of the player is the own block of the player, then the neighbours.
    assert_eq!(c[0], [0, 64, 0]);
    let d = |q: [i32; 3]| (0..3).map(|a| (f64::from(q[a]) + 0.5 - [0.5, 64.0, 0.5][a]).powi(2)).sum::<f64>();
    assert!(c.windows(2).all(|w| d(w[0]) <= d(w[1])));
}

#[test]
fn stacking_rules() {
    let f = |passengers, held, top: f64, near: f64| StackFacts { passengers, top_is_held: false, top_kind: "minecraft:cow".into(), top_area: top, held_area: held, distance_sq: near };
    assert_eq!(stack_decision(&f(0, 0.8, 1.2, 1.0), &no_tag), Stack::Stacked { near: true });
    assert_eq!(stack_decision(&f(0, 0.8, 1.2, 9.0), &no_tag), Stack::Stacked { near: false });
    // A bigger animal on a smaller one: refused (entitySizeMattersStacking).
    assert_eq!(stack_decision(&f(0, 2.0, 1.2, 1.0), &no_tag), Stack::Refused);
    // maxEntityStackLimit 10: eight passengers still fit, nine are the limit.
    assert_eq!(stack_decision(&f(8, 0.8, 1.2, 1.0), &no_tag), Stack::Stacked { near: true });
    assert_eq!(stack_decision(&f(9, 0.8, 1.2, 1.0), &no_tag), Stack::Refused);
    // A horse never holds a pile (nothing happens, no refusal sound).
    let h = StackFacts { top_kind: "minecraft:horse".into(), ..f(0, 0.8, 1.2, 1.0) };
    assert_eq!(stack_decision(&h, &no_tag), Stack::No);
    let me = StackFacts { top_is_held: true, ..f(0, 0.8, 1.2, 1.0) };
    assert_eq!(stack_decision(&me, &no_tag), Stack::No);
}

#[test]
fn carry_types_round_trip() {
    for t in [CarryType::Block, CarryType::Entity, CarryType::Player, CarryType::Invalid] {
        assert_eq!(CarryType::from_name(t.name()), t);
    }
    assert_eq!(CarryType::from_name("nonsense"), CarryType::Invalid);
    assert_eq!(CarryType::default(), CarryType::Invalid);
}
