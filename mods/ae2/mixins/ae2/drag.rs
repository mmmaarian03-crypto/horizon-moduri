fn ae2_drag(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    // No mouse-tweak drags over an AE2 window (its slots are not real slots).
    if game.ae2_screen_height().is_some() {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
