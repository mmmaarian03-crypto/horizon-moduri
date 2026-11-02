// The Lost Castle's map (LCMap.createMap) on top of the game's exploration map.
use super::trading::GameMaps;
use horizon_client::{items::ItemStack, merchant::ExplorationMap};
use horizon_world::worldgen::structure::start_checks::tlc::engine;

/// The stack an exploration-map trade made for `#tlc:lost_castle_map`: named
/// "Lost Castle Map" and, when no castle was found, a map of (0, 0) with its
/// target there (the mod logs an error and sells it anyway).
pub(super) fn castle_map(mut stack: ItemStack, maps: &GameMaps<'_>, map: &ExplorationMap) -> ItemStack {
    if map.destination != engine::MAP_DESTINATION {
        return stack;
    }
    if stack.data().is_none_or(|d| d.map_id.is_none()) {
        log::error!("Something went wrong with The Lost Castle");
        let id = maps.next_id.get();
        maps.next_id.set(id + 1);
        maps.made
            .borrow_mut()
            .push((id, [0, 0, 0], map.zoom, map.decoration.clone()));
        let d = stack.data_mut();
        d.map_id = Some(id);
        d.other.retain(|(k, _)| k != "minecraft:map_decorations");
        d.other.push((
            "minecraft:map_decorations".into(),
            horizon_world::nbt::Tag::compound([(
                "+",
                horizon_world::nbt::Tag::compound([
                    (
                        "type",
                        horizon_world::nbt::Tag::String(map.decoration.as_str().into()),
                    ),
                    ("x", horizon_world::nbt::Tag::Double(0f64.to_bits())),
                    ("z", horizon_world::nbt::Tag::Double(0f64.to_bits())),
                    ("rotation", horizon_world::nbt::Tag::Float(180.0f32.to_bits())),
                ]),
            )]),
        ));
    }
    stack.data_mut().custom_name = Some(engine::MAP_NAME.into());
    stack
}
