// While a list of the open book is shown, the window's typed keys go to its search bar (recipe_key).
fn patchouli_focused(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<bool> {
    if game.patchouli_typing() {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
