fn ae2_typing(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<bool> {
    // The terminal's search box takes the keys the way the anvil's rename box does.
    if game.ae2_typing() {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
