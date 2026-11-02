// The crosshair through compound blocks (their parts' boxes) and model-shaped blocks.
fn forja_pick(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<Option<Hit>> {
    match game.forja_pick() {
        Some(hit) => ::core::ops::ControlFlow::Break(hit),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
