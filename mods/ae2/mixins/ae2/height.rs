fn ae2_height(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<f32> {
    match game.ae2_screen_height() {
        Some(h) => ::core::ops::ControlFlow::Break(h),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
