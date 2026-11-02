// Oreberries Replanted's blocks and items in the content root (horizon_client::mod_content).
// - the Java classes the bushes and vats copy from their templates (the sweet berry bush,
//   the cauldron) become plain ones: the mod's blocks are plain `Block`s whose behaviour
//   is written in the mod's own hooks;
// - the bush items copy the sweet berries' item, which is food: the food goes;
// - the bushes are pushed by pistons like any block (they were `PushReaction.DESTROY` as sweet berry bushes).
use super::{J, Plan, Tag};
#[allow(dead_code)]
mod engine {
    include!("oreberriesreplanted_engine.rs");
}

/// A class name of a block's exported row: the plain one in its place.
pub(super) fn inert(name: &mut String) -> bool {
    match engine::inert_class(name) {
        Some(plain) => {
            *name = plain.to_owned();
            true
        }
        None => false,
    }
}

/// The gameplay row of one state of the block `id`.
pub(super) fn bush_row(id: &str, row: &mut J) {
    if engine::Ore::from_bush(id).is_some() {
        row.set("push", J::Str("PUSH_PULL".into()));
    }
}

/// `items.json` after the mods' items were copied in.
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    for ore in engine::ORES {
        if let Some(J::Obj(entry)) = map.get_mut(&ore.bush()) {
            entry.retain(|(k, _)| {
                !matches!(
                    k.as_str(),
                    "minecraft:food" | "minecraft:consumable" | "minecraft:compostable" | "minecraft:use_remainder"
                )
            });
        }
    }
}

/// The shape of a state in a shape table (the shape entry's `y` coordinates, the highest).
fn top_of(shapes: &[Tag], index: i32) -> Option<f64> {
    let Tag::LongArray(ys) = shapes.get(usize::try_from(index).ok()?)?.get("y")? else {
        return None;
    };
    ys.last().map(|bits| f64::from_bits(*bits as u64))
}

/// The collision of the bushes. The tables hold a shape number for each state, and the bush's
/// states copy the sweet berry bush's: no collision. Each age gets the number of a vanilla block
/// of about its size (`engine::collision_like`); the numbers are already in the table. Only the
/// collision tables are touched: a cactus is 15/16 high in them and 1 high in the occlusion
/// table, and a table that is neither is left as it is. The list of numbers is extended here, for
/// all the new blocks as `Plan::nbt` would, so that it finds the list already long enough.
pub(super) fn solid_bushes(plan: &Plan<'_>, tag: &mut Tag) {
    let extended = {
        let Some(Tag::IntArray(map)) = tag.get("cached_shapes") else {
            return;
        };
        if map.len() != plan.states {
            return;
        }
        let Some(Tag::List(shapes)) = tag.get("shapes") else {
            return;
        };
        let shape_of = |name: &str| -> Option<i32> {
            let state = *plan.defaults.get(name)?;
            map.get(state as usize).copied().filter(|i| *i >= 0)
        };
        let Some(cactus) = shape_of("minecraft:cactus").and_then(|i| top_of(shapes, i)) else {
            return;
        };
        if (cactus - 0.9375).abs() > 1.0e-9 {
            return;
        }
        let mut out = map.clone();
        for b in &plan.blocks {
            for id in b.rename.states.clone() {
                out.push(map[id as usize]);
            }
        }
        for b in &plan.blocks {
            if engine::Ore::from_bush(&b.decl.id).is_none() {
                continue;
            }
            for age in 0..b.rename.states.len() as i32 {
                if let Some(shape) = shape_of(engine::collision_like(age))
                    && let Some(slot) = out.get_mut((b.rename.first as i32 + age) as usize)
                {
                    *slot = shape;
                }
            }
        }
        out
    };
    if let Some(slot) = tag.get_mut("cached_shapes") {
        *slot = Tag::IntArray(extended);
    }
}
