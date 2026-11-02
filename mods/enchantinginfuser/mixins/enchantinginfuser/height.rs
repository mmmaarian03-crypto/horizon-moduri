fn enchantinginfuser_height(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<f32> {
    match game.infuser_screen_height() {
        Some(height) => ::core::ops::ControlFlow::Break(height),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
