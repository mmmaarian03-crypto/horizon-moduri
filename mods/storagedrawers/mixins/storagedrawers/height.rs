fn storagedrawers_height(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<f32> {
    // DrawerScreen.imageHeight for an open drawer screen.
    match game.storagedrawers_screen_height() {
        Some(h) => ::core::ops::ControlFlow::Break(h),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
