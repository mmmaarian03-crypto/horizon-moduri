// HEAD on `Overlay::write`: the mod's patterns, textures and trades also go where the registries read them.
fn additionalbanners_overlay(overlay: &Overlay<'_>, rel: &str, bytes: &[u8]) {
    additionalbanners::mirror(overlay, rel, bytes);
}
