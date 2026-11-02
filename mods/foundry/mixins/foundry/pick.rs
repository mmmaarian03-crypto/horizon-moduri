// The crosshair through compound blocks (their parts' boxes) and model-shaped blocks.
fn foundry_pick(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<Option<Hit>> {
    match game.foundry_pick() {
        Some(hit) => ::core::ops::ControlFlow::Break(hit),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
