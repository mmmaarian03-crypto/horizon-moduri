// The overlays' lines, with the fishing line's.
fn moreoverlays_lines(
    game: &crate::game::Game<'_>,
    alpha: f64,
    size: [u32; 2],
) -> ::core::ops::ControlFlow<Vec<Line>> {
    match game.moreoverlays_fishing(alpha, size) {
        Some(lines) => ::core::ops::ControlFlow::Break(lines),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
