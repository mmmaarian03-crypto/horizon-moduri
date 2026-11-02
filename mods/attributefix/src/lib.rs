// AttributeFix for Horizon: the rules, without any game code.
// Written from the behaviour of AttributeFix 21.0.5 for Minecraft 1.20.1 (Darkhax, LGPL-2.1,
// https://github.com/Darkhax-Minecraft/AttributeFix, branch 1.20.1, read for private use only)
// and from the pack's own config file `attributefix.json`; no code of the original is used.
//
// What the original does: when the game finishes loading it reads `attributefix.json`, which
// lists every ranged attribute with its `min` and `max`, and writes each configured minimum and
// maximum into the attribute (`RangedAttribute.minValue/maxValue`). It never looks at the
// `enabled` flag (`Entry.isEnabled` is not called anywhere), and an entry whose minimum is above
// its maximum is refused. An id that is not an attribute is ignored.
// Horizon keeps the same ranges in the definitions it builds from its attribute catalog
// (`attributes.json`, read once when the game loads: `Definition::ranged(default, min, max)`);
// `adjust` gives the range such a definition gets instead.

/// The pack's `attributefix.json` entry of one attribute: its id in 1.20.1, the matching
/// attribute of Horizon (Minecraft renamed `generic.*`, `horse.*` and `zombie.*` in 1.21.2), the
/// `default` the file recorded for the minimum and the maximum, and their `value`.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub id: &'static str,
    pub horizon: &'static str,
    pub default_min: f64,
    pub default_max: f64,
    pub min: f64,
    pub max: f64,
}

impl Entry {
    /// Whether the pack's file asks for a range other than the one it recorded. Only those
    /// entries change anything on Horizon: an entry equal to its own `default` just writes back
    /// the number the 1.20.1 game already had (for example `horse.jump_strength` 0..2, where
    /// Horizon's vanilla range is 0..32), and Horizon keeps its own range there.
    pub fn is_change(&self) -> bool {
        self.min != self.default_min || self.max != self.default_max
    }
}

/// The 18 `minecraft:` attributes of the pack's `attributefix.json` (the other 135 entries are
/// attributes of mods that Horizon does not have: obscure_api, attributeslib, irons_spellbooks,
/// goety, tconstruct, ... and Forge's own `forge:*`, so they have nothing to apply to).
/// Source: `the 1.20.1 pack\config\attributefix.json`, `enabled`, `min.value`, `max.value`.
pub const CONFIG: &[Entry] = &[
    Entry { id: "minecraft:air_drag_modifier", horizon: "minecraft:air_drag_modifier", default_min: 0.0, default_max: 2048.0, min: 0.0, max: 2048.0 },
    Entry { id: "minecraft:bounciness", horizon: "minecraft:bounciness", default_min: 0.0, default_max: 1.0, min: 0.0, max: 1.0 },
    Entry { id: "minecraft:friction_modifier", horizon: "minecraft:friction_modifier", default_min: 0.0, default_max: 2048.0, min: 0.0, max: 2048.0 },
    Entry { id: "minecraft:generic.armor", horizon: "minecraft:armor", default_min: 0.0, default_max: 200.0, min: 0.0, max: 1000000.0 },
    Entry { id: "minecraft:generic.armor_toughness", horizon: "minecraft:armor_toughness", default_min: 0.0, default_max: 100.0, min: 0.0, max: 1000000.0 },
    Entry { id: "minecraft:generic.attack_damage", horizon: "minecraft:attack_damage", default_min: 0.0, default_max: 2048.0, min: 0.0, max: 1000000.0 },
    Entry { id: "minecraft:generic.attack_knockback", horizon: "minecraft:attack_knockback", default_min: 0.0, default_max: 5.0, min: 0.0, max: 1000000.0 },
    Entry { id: "minecraft:generic.attack_speed", horizon: "minecraft:attack_speed", default_min: 0.0, default_max: 1024.0, min: 0.0, max: 1024.0 },
    Entry { id: "minecraft:generic.flying_speed", horizon: "minecraft:flying_speed", default_min: 0.0, default_max: 1024.0, min: 0.0, max: 1024.0 },
    Entry { id: "minecraft:generic.follow_range", horizon: "minecraft:follow_range", default_min: 0.0, default_max: 2048.0, min: 0.0, max: 2048.0 },
    Entry { id: "minecraft:generic.knockback_resistance", horizon: "minecraft:knockback_resistance", default_min: -2.0, default_max: 1.0, min: -2.0, max: 1.0 },
    Entry { id: "minecraft:generic.luck", horizon: "minecraft:luck", default_min: -1024.0, default_max: 1024.0, min: -1024.0, max: 1024.0 },
    Entry { id: "minecraft:generic.max_health", horizon: "minecraft:max_health", default_min: 1.0, default_max: 1024.0, min: 1.0, max: 1000000.0 },
    Entry { id: "minecraft:generic.movement_speed", horizon: "minecraft:movement_speed", default_min: 0.0, default_max: 1024.0, min: 0.0, max: 1024.0 },
    Entry { id: "minecraft:horse.jump_strength", horizon: "minecraft:jump_strength", default_min: 0.0, default_max: 2.0, min: 0.0, max: 2.0 },
    Entry { id: "minecraft:waypoint_receive_range", horizon: "minecraft:waypoint_receive_range", default_min: 0.0, default_max: 60000000.0, min: 0.0, max: 60000000.0 },
    Entry { id: "minecraft:waypoint_transmit_range", horizon: "minecraft:waypoint_transmit_range", default_min: 0.0, default_max: 60000000.0, min: 0.0, max: 60000000.0 },
    Entry { id: "minecraft:zombie.spawn_reinforcements", horizon: "minecraft:spawn_reinforcements", default_min: 0.0, default_max: 1.0, min: 0.0, max: 1.0 },
];

/// The ranged attributes of Horizon's 26.3 catalog (`attributes.json` of the game's cache, which
/// lists `BuiltInRegistries.ATTRIBUTE`): id, default, minimum, maximum.
///
/// Horizon's loader builds each attribute with `Definition::ranged(default, min, max)` and does not
/// pass the id, so the rules recognise an attribute by these three numbers. They are different
/// for every attribute of the catalog (a test checks it); a definition that matches none, or more
/// than one, is left alone.
pub const HORIZON_ATTRIBUTES: &[(&str, f64, f64, f64)] = &[
    ("minecraft:air_drag_modifier", 1.0, 0.0, 2048.0),
    ("minecraft:armor", 0.0, 0.0, 30.0),
    ("minecraft:armor_toughness", 0.0, 0.0, 20.0),
    ("minecraft:attack_damage", 2.0, 0.0, 2048.0),
    ("minecraft:attack_knockback", 0.0, 0.0, 5.0),
    ("minecraft:attack_speed", 4.0, 0.0, 1024.0),
    ("minecraft:below_name_distance", 10.0, 0.0, 512.0),
    ("minecraft:block_break_speed", 1.0, 0.0, 1024.0),
    ("minecraft:block_interaction_range", 4.5, 0.0, 64.0),
    ("minecraft:bounciness", 0.0, 0.0, 1.0),
    ("minecraft:burning_time", 1.0, 0.0, 1024.0),
    ("minecraft:camera_distance", 4.0, 0.0, 32.0),
    ("minecraft:explosion_knockback_resistance", 0.0, 0.0, 1.0),
    ("minecraft:entity_interaction_range", 3.0, 0.0, 64.0),
    ("minecraft:fall_damage_multiplier", 1.0, 0.0, 100.0),
    ("minecraft:flying_speed", 0.4, 0.0, 1024.0),
    ("minecraft:follow_range", 32.0, 0.0, 2048.0),
    ("minecraft:friction_modifier", 1.0, 0.0, 2048.0),
    ("minecraft:gravity", 0.08, -1.0, 1.0),
    ("minecraft:jump_strength", 0.41999998688697815, 0.0, 32.0),
    ("minecraft:knockback_resistance", 0.0, -2.0, 1.0),
    ("minecraft:luck", 0.0, -1024.0, 1024.0),
    ("minecraft:max_absorption", 0.0, 0.0, 2048.0),
    ("minecraft:max_health", 20.0, 1.0, 1024.0),
    ("minecraft:mining_efficiency", 0.0, 0.0, 1024.0),
    ("minecraft:movement_efficiency", 0.0, 0.0, 1.0),
    ("minecraft:movement_speed", 0.7, 0.0, 1024.0),
    ("minecraft:name_tag_distance", 64.0, 0.0, 512.0),
    ("minecraft:oxygen_bonus", 0.0, 0.0, 1024.0),
    ("minecraft:safe_fall_distance", 3.0, -1024.0, 1024.0),
    ("minecraft:scale", 1.0, 0.0625, 16.0),
    ("minecraft:sneaking_speed", 0.3, 0.0, 1.0),
    ("minecraft:spawn_reinforcements", 0.0, 0.0, 1.0),
    ("minecraft:step_height", 0.6, 0.0, 10.0),
    ("minecraft:submerged_mining_speed", 0.2, 0.0, 20.0),
    ("minecraft:sweeping_damage_ratio", 0.0, 0.0, 1.0),
    ("minecraft:tempt_range", 10.0, 0.0, 2048.0),
    ("minecraft:water_movement_efficiency", 0.0, 0.0, 1.0),
    ("minecraft:waypoint_transmit_range", 0.0, 0.0, 60000000.0),
    ("minecraft:waypoint_receive_range", 0.0, 0.0, 60000000.0),
];

/// The Horizon attribute that has exactly these default, minimum and maximum.
pub fn attribute_of(default: f64, min: f64, max: f64) -> Option<&'static str> {
    let mut hits = HORIZON_ATTRIBUTES
        .iter()
        .filter(|a| a.1 == default && a.2 == min && a.3 == max);
    let first = hits.next()?;
    hits.next().is_none().then_some(first.0)
}

/// `AttributeConfig.applyChanges` for one definition the loader is building: the minimum and
/// maximum the pack's file gives that attribute, or `None` to build it as it is (not a configured
/// attribute, the file asks for the range it already has, an entry whose minimum is above its
/// maximum, or a range that leaves out the attribute's default, which Horizon's loader refuses and
/// would stop the game from starting).
pub fn adjust(default: f64, min: f64, max: f64) -> Option<(f64, f64)> {
    let id = attribute_of(default, min, max)?;
    let e = CONFIG.iter().find(|e| e.horizon == id && e.is_change())?;
    // "was configured to have a minimum value higher than its maximum. This is not permitted!"
    if e.min > e.max || default < e.min || default > e.max {
        return None;
    }
    ((e.min, e.max) != (min, max)).then_some((e.min, e.max))
}
