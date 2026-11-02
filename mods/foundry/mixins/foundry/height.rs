fn foundry_height(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<f32> {
    match game.foundry_screen_height() {
        Some(height) => ::core::ops::ControlFlow::Break(height),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
