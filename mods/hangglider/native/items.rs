// Hang Glider's items in the content root (horizon_client::mod_content). The two gliders copy the
// components of `minecraft:paper`; this makes them the original's plain `Item` with durability
// (`ModRegistry`: 818 / 2202, stack 1) repaired by leather / phantom membrane (`GliderItem.Type`).
use super::J;
#[allow(dead_code)]
mod engine {
    include!("hangglider_engine.rs");
}

/// `items.json` after the mods' items were copied in.
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    for (id, ops) in engine::item_ops() {
        let Some(entry) = map.get_mut(id) else {
            continue;
        };
        for (path, v) in ops {
            let value = match v {
                engine::Val::Int(i) => J::Num(i.into()),
                engine::Val::Str(s) => J::Str(s.to_owned()),
            };
            match path {
                [key] => entry.set(key, value),
                [key, sub] => {
                    if entry.get_mut(key).is_none() {
                        entry.set(key, J::Obj(Vec::new()));
                    }
                    if let Some(o) = entry.get_mut(key) {
                        o.set(sub, value);
                    }
                }
                _ => {}
            }
        }
    }
}
