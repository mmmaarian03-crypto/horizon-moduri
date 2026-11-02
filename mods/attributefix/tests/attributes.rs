use horizon_attributefix::{CONFIG, HORIZON_ATTRIBUTES, adjust, attribute_of};
use std::collections::BTreeMap;

/// What Horizon's loader would build, attribute by attribute: each definition passes through
/// `adjust` with its vanilla (default, min, max), as `Definition::ranged` is called.
fn built() -> BTreeMap<&'static str, (f64, f64, f64)> {
    HORIZON_ATTRIBUTES
        .iter()
        .map(|&(id, d, lo, hi)| {
            let (lo, hi) = adjust(d, lo, hi).unwrap_or((lo, hi));
            (id, (d, lo, hi))
        })
        .collect()
}

#[test]
fn the_five_raised_caps_are_the_pack_values() {
    let c = built();
    // attributefix.json: max_health, armor, armor_toughness, attack_damage, attack_knockback -> 1000000.
    assert_eq!(c["minecraft:max_health"], (20.0, 1.0, 1_000_000.0));
    assert_eq!(c["minecraft:armor"], (0.0, 0.0, 1_000_000.0));
    assert_eq!(c["minecraft:armor_toughness"], (0.0, 0.0, 1_000_000.0));
    assert_eq!(c["minecraft:attack_damage"], (2.0, 0.0, 1_000_000.0));
    assert_eq!(c["minecraft:attack_knockback"], (0.0, 0.0, 1_000_000.0));
}

#[test]
fn what_the_changes_replace_is_vanillas_range() {
    assert_eq!(adjust(0.0, 0.0, 30.0), Some((0.0, 1_000_000.0)), "armor 0..30");
    assert_eq!(adjust(0.0, 0.0, 20.0), Some((0.0, 1_000_000.0)), "armor_toughness 0..20");
    assert_eq!(adjust(2.0, 0.0, 2048.0), Some((0.0, 1_000_000.0)), "attack_damage 0..2048");
    assert_eq!(adjust(0.0, 0.0, 5.0), Some((0.0, 1_000_000.0)), "attack_knockback 0..5");
    assert_eq!(adjust(20.0, 1.0, 1024.0), Some((1.0, 1_000_000.0)), "max_health 1..1024");
}

#[test]
fn every_other_attribute_keeps_its_vanilla_range() {
    let c = built();
    let raised = [
        "minecraft:max_health",
        "minecraft:armor",
        "minecraft:armor_toughness",
        "minecraft:attack_damage",
        "minecraft:attack_knockback",
    ];
    for &(id, d, lo, hi) in HORIZON_ATTRIBUTES {
        if !raised.contains(&id) {
            assert_eq!(c[id], (d, lo, hi), "{id} must not change");
        }
    }
    assert_eq!(c.len(), HORIZON_ATTRIBUTES.len());
}

#[test]
fn horse_jump_strength_is_not_lowered() {
    // The file lists horse.jump_strength 0..2 as both its default and its value: the 1.20.1 range,
    // written back unchanged. Horizon's jump_strength is 0..32 and stays so.
    let c = built();
    assert_eq!(c["minecraft:jump_strength"].1..=c["minecraft:jump_strength"].2, 0.0..=32.0);
}

#[test]
fn every_default_stays_inside_its_range() {
    // Horizon refuses a catalog with a default outside its range, so the game would not start.
    for (id, (d, lo, hi)) in built() {
        assert!(lo <= d && d <= hi, "{id}: {d} not in {lo}..{hi}");
        assert!(lo <= hi, "{id}: min above max");
    }
}

#[test]
fn a_second_pass_finds_nothing_to_change() {
    // The loader builds a definition once; the rules must also be stable if it asks again with
    // the numbers it got (the hook calls the loader again with the new range).
    for (_, (d, lo, hi)) in built() {
        assert_eq!(adjust(d, lo, hi), None);
    }
}

#[test]
fn the_changed_attributes_are_told_apart_by_default_min_and_max() {
    // The loader does not pass the id, so the rules recognise an attribute by its three numbers.
    // Some attributes share them (air_drag_modifier and friction_modifier are both 1, 0..2048):
    // those are never configured to change, and `attribute_of` answers `None` for them.
    for e in CONFIG.iter().filter(|e| e.is_change()) {
        let (_, d, lo, hi) = *HORIZON_ATTRIBUTES.iter().find(|a| a.0 == e.horizon).unwrap();
        assert_eq!(attribute_of(d, lo, hi), Some(e.horizon), "{} is not unique", e.horizon);
    }
    assert_eq!(attribute_of(1.0, 0.0, 2048.0), None, "air_drag_modifier / friction_modifier");
    assert_eq!(attribute_of(0.0, 0.0, 30.0), Some("minecraft:armor"));
    assert_eq!(attribute_of(7.0, 0.0, 30.0), None);
    for (i, a) in HORIZON_ATTRIBUTES.iter().enumerate() {
        for b in &HORIZON_ATTRIBUTES[i + 1..] {
            assert_ne!(a.0, b.0, "duplicate id");
        }
    }
}

#[test]
fn a_shared_set_of_numbers_is_left_alone() {
    // Nothing is guessed when two attributes look the same.
    assert_eq!(adjust(1.0, 0.0, 2048.0), None);
}

#[test]
fn the_config_table_matches_the_file() {
    // 18 `minecraft:` entries; the five raised ones are the only entries that differ from their default.
    assert_eq!(CONFIG.len(), 18);
    let changing: Vec<_> = CONFIG.iter().filter(|e| e.is_change()).map(|e| e.id).collect();
    assert_eq!(
        changing,
        [
            "minecraft:generic.armor",
            "minecraft:generic.armor_toughness",
            "minecraft:generic.attack_damage",
            "minecraft:generic.attack_knockback",
            "minecraft:generic.max_health",
        ]
    );
    for e in CONFIG {
        assert!(e.min <= e.max, "{} has min above max", e.id);
        // Every Horizon id the table names is a real attribute of Horizon's 26.3 catalog.
        assert!(
            HORIZON_ATTRIBUTES.iter().any(|v| v.0 == e.horizon),
            "{} -> {} is not in the catalog",
            e.id,
            e.horizon
        );
    }
}

#[test]
fn the_changes_use_the_attributes_that_exist_on_horizon() {
    // The renamed ids of 1.21.2: generic.* lost its prefix.
    let ids: BTreeMap<_, _> = CONFIG.iter().map(|e| (e.id, e.horizon)).collect();
    assert_eq!(ids["minecraft:generic.max_health"], "minecraft:max_health");
    assert_eq!(ids["minecraft:generic.armor"], "minecraft:armor");
    assert_eq!(ids["minecraft:horse.jump_strength"], "minecraft:jump_strength");
    assert_eq!(ids["minecraft:zombie.spawn_reinforcements"], "minecraft:spawn_reinforcements");
}
