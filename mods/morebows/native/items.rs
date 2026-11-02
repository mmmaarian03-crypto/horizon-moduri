// More Bows' items in the content root (horizon_client::mod_content). The eight bows copy the components of
// `minecraft:bow`; this sets the numbers of the original (`MoreBows.getAllItems`, `ConfigBows`):
// durability, the repair material (`CustomBow.isValidRepairItem`) and, for the flame bow,
// `fireResistant()`. A custom bow is not `Items.BOW`, so it is not a furnace fuel as the bow is.
use super::J;
#[allow(dead_code)]
mod engine {
    include!("morebows_engine.rs");
}

/// `items.json` after the mods' items were copied in: their components get the original's numbers.
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    for n in engine::item_numbers() {
        let Some(entry) = map.get_mut(&n.id) else {
            continue;
        };
        entry.set("minecraft:max_damage", J::Num(n.max_damage.into()));
        let repair: Vec<J> = n.repair.iter().map(|r| J::Str((*r).to_owned())).collect();
        entry.set(
            "minecraft:repairable",
            J::Obj(vec![("items".to_owned(), J::Arr(repair))]),
        );
        if n.fire_resistant {
            entry.set(
                "minecraft:damage_resistant",
                J::Obj(vec![("types".to_owned(), J::Str("#minecraft:is_fire".to_owned()))]),
            );
        }
        if let J::Obj(o) = entry {
            o.retain(|(k, _)| k != "minecraft:cooking_fuel");
        }
    }
}
