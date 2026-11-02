fn curios_piglins(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<bool> {
    if game.curios_wearing_gold() { ::core::ops::ControlFlow::Break(true) } else { ::core::ops::ControlFlow::Continue(()) }
}
