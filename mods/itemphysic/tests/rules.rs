use horizon_itemphysic::*;
use std::collections::{BTreeMap, BTreeSet};

/// A small catalog: item -> block it places; block tags; fuels.
#[derive(Default)]
struct Catalog {
    blocks: BTreeMap<&'static str, &'static str>,
    tags: BTreeMap<&'static str, BTreeSet<&'static str>>,
    fuels: BTreeSet<&'static str>,
}
impl ItemFacts for Catalog {
    fn block_of(&self, item: &str) -> Option<String> {
        self.blocks.get(item).map(|b| b.to_string())
    }
    fn block_in_tag(&self, tag: &str, block: &str) -> bool {
        self.tags.get(tag).is_some_and(|t| t.contains(block))
    }
    fn is_fuel(&self, item: &str) -> bool {
        self.fuels.contains(item)
    }
}
fn catalog() -> Catalog {
    let mut c = Catalog::default();
    for (i, b) in [
        ("minecraft:oak_log", "minecraft:oak_log"),
        ("minecraft:wheat_seeds", "minecraft:wheat"),
        ("minecraft:string", "minecraft:tripwire"),
        ("minecraft:stone", "minecraft:stone"),
        ("minecraft:obsidian", "minecraft:obsidian"),
        ("minecraft:white_wool", "minecraft:white_wool"),
        ("minecraft:crafting_table", "minecraft:crafting_table"),
    ] {
        c.blocks.insert(i, b);
    }
    c.tags.insert("minecraft:logs", ["minecraft:oak_log"].into());
    c.tags.insert("minecraft:logs_that_burn", ["minecraft:oak_log"].into());
    c.tags.insert("minecraft:mineable/axe", ["minecraft:oak_log", "minecraft:crafting_table"].into());
    c.tags.insert("minecraft:wool", ["minecraft:white_wool"].into());
    c.fuels.insert("minecraft:coal");
    c.fuels.insert("minecraft:stick");
    c
}

#[test]
fn tuned_lists_classify_items() {
    let c = catalog();
    // Wood swims and burns; the crafting table is in mineable/axe.
    assert_eq!(classify("minecraft:oak_log", &c), Class { swim: true, burn: true, undestroyable: false });
    assert!(classify("minecraft:crafting_table", &c).swim);
    // Block filters match the block item: wheat seeds place minecraft:wheat, string places the tripwire.
    assert!(classify("minecraft:wheat_seeds", &c).swim && classify("minecraft:wheat_seeds", &c).burn);
    assert!(classify("minecraft:string", &c).swim);
    // Stone neither swims nor burns: it sinks and survives lava.
    assert_eq!(classify("minecraft:stone", &c), Class::default());
    // Wool burns but does not swim.
    let wool = classify("minecraft:white_wool", &c);
    assert!(wool.burn && !wool.swim);
    // Fuels burn (`{id:"fuel"}` is only in the burning list).
    assert!(classify("minecraft:coal", &c).burn && !classify("minecraft:coal", &c).swim);
    // Plain items of the lists.
    assert!(classify("minecraft:bow", &c).swim && classify("minecraft:bow", &c).burn);
    assert!(!classify("minecraft:paper", &c).swim && classify("minecraft:paper", &c).burn);
    // The undestroyable list.
    assert!(classify("minecraft:nether_star", &c).undestroyable);
    assert!(classify("minecraft:obsidian", &c).undestroyable);
    assert!(!classify("minecraft:diamond", &c).undestroyable);
    for b in 0..8u8 {
        assert_eq!(Class::from_bits(b).bits(), b);
    }
}

#[test]
fn the_lists_are_tuned_config() {
    assert_eq!(config::SWIMMING.len(), 64);
    assert_eq!(config::BURNING.len(), 83);
    assert_eq!(config::UNDESTROYABLE.len(), 4);
    assert!(config::BURNING.contains(&Filter::Fuel));
    assert!(!config::SWIMMING.contains(&Filter::Fuel));
    assert!(config::GENERAL_FALL_SOUNDS && config::GENERAL_DISABLE_CACTUS_DAMAGE && !config::GENERAL_VANILLA_FLOW_BEHAVIOR);
}

#[test]
fn gravity_swimming_and_sinking() {
    // Air: gravity 0.04 (ItemPhysic adds it itself; vanilla's branch is off).
    assert!((pre_motion(0.0, None, false) + 0.04).abs() < 1e-12);
    // Water, a swimming item rises up to 0.1 per tick, by at most 0.04 a tick.
    assert!((pre_motion(-0.2, Some(InFluid::Water), true) - (-0.16)).abs() < 1e-12);
    assert!((pre_motion(0.08, Some(InFluid::Water), true) - 0.1).abs() < 1e-12);
    // At 0.1 or more the force stays -0.02.
    assert!((pre_motion(0.1, Some(InFluid::Water), true) - 0.08).abs() < 1e-12);
    // Water, a sinking item: -0.02 a tick, and falling faster than 0.1 is braked (* 0.8).
    assert!((pre_motion(0.0, Some(InFluid::Water), false) + 0.02).abs() < 1e-12);
    assert!((pre_motion(-0.2, Some(InFluid::Water), false) + 0.16).abs() < 1e-12);
    // Lava: viscosity 6, nothing swims in it.
    assert!((pre_motion(0.0, Some(InFluid::Lava), true) + 0.02 / 6.0).abs() < 1e-12);
    // A sinking item in still water settles at about -0.1 a tick.
    let mut vy = 0.0;
    for _ in 0..200 {
        vy = pre_motion(vy, Some(InFluid::Water), false);
    }
    assert!(vy < -0.09 && vy > -0.13, "{vy}");
    // A swimming item floats up at 0.1.
    let mut vy = -0.3;
    for _ in 0..50 {
        vy = pre_motion(vy, Some(InFluid::Water), true);
    }
    assert!((vy - 0.1).abs() < 0.03, "{vy}");
}

#[test]
fn drag_on_the_ground_in_air_and_in_fluids() {
    // Air, not on the ground: (0.98, 0.98, 0.98).
    let v = post_motion([1.0, -1.0, 1.0], false, 0.6, None);
    assert!((v[0] - 0.98).abs() < 1e-6 && (v[1] + 0.98).abs() < 1e-9);
    // On the ground: block friction * 0.98 and the -0.5 bounce.
    let v = post_motion([1.0, -1.0, 1.0], true, 0.6, None);
    assert!((v[0] - f64::from(0.6f32 * 0.98)).abs() < 1e-9 && (v[1] - 0.49).abs() < 1e-9);
    // Ice keeps its glide.
    assert!(post_motion([1.0, 0.0, 0.0], true, 0.98, None)[0] > 0.95);
    // Water: horizontal * 1/1.2, vertical untouched; lava * 1/7.2.
    let v = post_motion([1.2, -0.3, 1.2], false, 0.6, Some(InFluid::Water));
    assert!((v[0] - 1.0).abs() < 1e-12 && (v[1] + 0.3).abs() < 1e-12);
    let v = post_motion([7.2, 0.0, 0.0], false, 0.6, Some(InFluid::Lava));
    assert!((v[0] - 1.0).abs() < 1e-12);
}

#[test]
fn the_fluid_counts_from_02_below_its_surface() {
    // A full block of water (height 1): items up to 1.2 above the block bottom are in it.
    assert_eq!(in_fluid(Some(InFluid::Water), 64.9, 64, 1.0), Some(InFluid::Water));
    // A thin flow (height 1/9): an item lying 0.5 into the block is not.
    assert_eq!(in_fluid(Some(InFluid::Water), 64.5, 64, 1.0 / 9.0), None);
    assert_eq!(in_fluid(None, 64.0, 64, 1.0), None);
}

#[test]
fn only_items_that_burn_burn() {
    assert!(burns(Some(InFluid::Lava), 0.2, false, true));
    assert!(!burns(Some(InFluid::Lava), 0.2, false, false), "stone survives lava");
    assert!(!burns(Some(InFluid::Lava), 0.05, false, true), "lava below the eyes (0.1014)");
    assert!(!burns(None, 1.0, true, true), "only in a fluid");
    let stone = Class::default();
    let wood = Class { swim: true, burn: true, undestroyable: false };
    let star = Class { swim: false, burn: false, undestroyable: true };
    assert!(!may_hurt(stone, Source::Lava) && !may_hurt(stone, Source::Fire));
    assert!(may_hurt(wood, Source::Lava) && may_hurt(wood, Source::Fire));
    assert!(!may_hurt(wood, Source::Cactus), "disableCactusDamage");
    assert!(may_hurt(stone, Source::Explosion));
    for s in [Source::Fire, Source::Lava, Source::Cactus, Source::Explosion, Source::Other] {
        assert!(!may_hurt(star, s));
    }
}

#[test]
fn falling_items_make_a_sound_when_they_land() {
    let mut d = 0.0f32;
    for _ in 0..10 {
        d = fall(d, -0.5, false, false).0;
    }
    assert!((d - 5.0).abs() < 1e-5);
    let (after, sound) = fall(d, 0.0, true, false);
    assert_eq!(after, 0.0);
    assert!((sound.unwrap() - 0.5).abs() < 1e-6);
    assert_eq!(fall(30.0, 0.0, true, false).1, Some(1.0), "volume capped at 1");
    assert_eq!(fall(0.0, 0.0, true, false).1, None, "no fall, no sound");
    assert_eq!(fall(5.0, -0.5, false, true), (0.0, None), "water resets the fall");
    assert_eq!(FALL_SOUND, "minecraft:block.wool.fall");
}
