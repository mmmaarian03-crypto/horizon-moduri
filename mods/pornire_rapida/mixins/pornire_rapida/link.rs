// HEAD on `Overlay::link_children`: on Windows the base's files are hard-linked into the overlay instead of copied.
fn pornire_rapida_link(overlay: &Overlay<'_>, rel: &Path) -> core::ops::ControlFlow<Result<()>> {
    pornire_rapida::link_children(overlay, rel)
}
