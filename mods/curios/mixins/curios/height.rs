fn curios_height(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<f32> {
    match game.curios_screen_height() {
        Some(h) => ::core::ops::ControlFlow::Break(h),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
