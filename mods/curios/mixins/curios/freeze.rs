fn curios_freeze(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<bool> {
    if game.curios_freeze_immune() { ::core::ops::ControlFlow::Break(true) } else { ::core::ops::ControlFlow::Continue(()) }
}
