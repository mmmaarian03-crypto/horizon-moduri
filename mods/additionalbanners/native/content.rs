// AdditionalBanners in the content root builder (`crates/horizon-client/src/mod_content.rs`, library crate).
// tools/bundle_extra.py pastes src/lib.rs in place of the include! and writes mixins/additionalbanners/content_runtime.rs.
#[allow(dead_code)]
mod engine {
    include!("additionalbanners_engine.rs");
}
use super::{J, Overlay};

/// HEAD on `Overlay::write`: the pattern definitions, their banner/shield textures and the trades the carry
/// holds in the mod's namespace are written a second time where `Banners::load`, the atlas, the registry
/// sync and `Trades::load` read them (`data/minecraft/...`, `assets/minecraft/...`). New files only: nothing
/// of vanilla's is replaced (the names start with `additionalbanners`).
pub(super) fn mirror(overlay: &Overlay<'_>, rel: &str, bytes: &[u8]) {
    let Some(to) = engine::mirror(rel) else {
        return;
    };
    // Re-enters this hook with a minecraft path, which `mirror` leaves alone. A failed copy only leaves the
    // pattern out of the registry (no layer drawn), as without the mod.
    let _ = overlay.write(&to, bytes);
}

/// RETURN on `Plan::items` (items.json): each item gives the loom its own patterns
/// (`BannerPatternItem(TagKey<BannerPattern>)` of `Content.createPattern`), not the template's creeper.
pub(super) fn patch_items(map: &mut J, components: bool) {
    if !components {
        return;
    }
    for (name, _) in engine::ITEMS {
        let id = format!("{}:{name}", engine::NS);
        let Some(tag) = engine::pattern_tag(&id) else {
            continue;
        };
        if let Some(entry) = map.get_mut(&id) {
            entry.set("minecraft:provides_banner_patterns", J::Str(tag));
        }
    }
}
