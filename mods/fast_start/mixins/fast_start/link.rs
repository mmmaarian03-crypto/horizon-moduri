// HEAD on `Overlay::link_children`: on Windows the base's files are hard-linked into the overlay instead of copied.
fn fast_start_link(overlay: &Overlay<'_>, rel: &Path) -> core::ops::ControlFlow<Result<()>> {
    fast_start::link_children(overlay, rel)
}
