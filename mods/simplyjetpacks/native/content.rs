// Simply Jetpacks 2 in the content root builder (`crates/horizon-client/src/mod_content.rs`, library crate).
// tools/bundle_extra.py pastes src/lib.rs in place of the include! and writes mixins/simplyjetpacks/content_runtime.rs.
#[allow(dead_code)]
mod engine {
    include!("simplyjetpacks_engine.rs");
}
use super::J;

fn num(v: f64) -> J {
    J::f64(v)
}
fn modifier(kind: &str, amount: f64, slot: &str) -> J {
    J::Obj(vec![
        ("type".into(), J::Str(kind.into())),
        ("amount".into(), num(amount)),
        ("id".into(), J::Str(format!("simplyjetpacks:armor.{slot}"))),
        ("operation".into(), J::Str("add_value".into())),
        ("slot".into(), J::Str(slot.into())),
    ])
}
fn remove(entry: &mut J, keys: &[&str]) {
    if let J::Obj(fields) = entry {
        fields.retain(|(k, _)| !keys.contains(&k.as_str()));
    }
}
/// The worn look: `asset_id` of the equipment asset, or none (the jetpack's own model is not drawn here).
fn equippable(slot: &str, asset: Option<&str>, sound: &str) -> J {
    let mut v = vec![];
    if let Some(a) = asset {
        v.push(("asset_id".into(), J::Str(a.into())));
    }
    v.push(("equip_sound".into(), J::Str(sound.into())));
    v.push(("slot".into(), J::Str(slot.into())));
    J::Obj(v)
}

/// RETURN on `Plan::items` (items.json): the template's iron/gold armour numbers become the original's
/// (`JetpackArmorMaterial`: durability 0 = never wears, defense of the chest piece, toughness, knockback resistance,
/// enchantability + `JetpackType.getEnchantability`; the creative jetpacks glint, `isFoil`).
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    for k in engine::KINDS {
        let id = format!("{}:{}", engine::NS, k.item);
        let Some(entry) = map.get_mut(&id) else {
            continue;
        };
        let (defense, toughness, knockback, _) = k.material.numbers();
        let mut mods = vec![
            modifier("minecraft:armor", defense, "chest"),
            modifier("minecraft:armor_toughness", toughness, "chest"),
        ];
        if knockback > 0.0 {
            mods.push(modifier("minecraft:knockback_resistance", knockback, "chest"));
        }
        remove(entry, &["minecraft:max_damage", "minecraft:damage", "minecraft:repairable"]);
        entry.set("minecraft:attribute_modifiers", J::Arr(mods));
        let sound = if k.potato() { "minecraft:item.armor.equip_leather" } else { "minecraft:item.armor.equip_iron" };
        entry.set("minecraft:equippable", equippable("chest", None, sound));
        let ench = k.enchantability();
        if ench > 0 {
            entry.set("minecraft:enchantable", J::Obj(vec![("value".into(), num(f64::from(ench)))]));
        } else {
            remove(entry, &["minecraft:enchantable"]);
        }
        if k.creative() {
            entry.set("minecraft:enchantment_glint_override", J::Bool(true));
        }
    }
    // PilotGogglesItem: PILOT_GOGGLES material (no defense, no durability, not enchantable), worn as the carried
    // equipment assets (pilot_goggles_gold / pilot_goggles_silver).
    for (item, asset) in [("pilot_goggles_gold", "pilot_goggles_gold"), ("pilot_goggles_iron", "pilot_goggles_silver")] {
        let id = format!("{}:{item}", engine::NS);
        let Some(entry) = map.get_mut(&id) else {
            continue;
        };
        remove(entry, &["minecraft:max_damage", "minecraft:damage", "minecraft:repairable", "minecraft:enchantable"]);
        entry.set(
            "minecraft:attribute_modifiers",
            J::Arr(vec![modifier("minecraft:armor", 0.0, "head"), modifier("minecraft:armor_toughness", 0.0, "head")]),
        );
        let asset = format!("{}:{asset}", engine::NS);
        entry.set("minecraft:equippable", equippable("head", Some(&asset), "minecraft:item.armor.equip_leather"));
    }
}
