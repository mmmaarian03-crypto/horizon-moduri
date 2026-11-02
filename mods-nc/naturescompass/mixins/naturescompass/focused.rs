// While the list's search field has the focus, the window's typed keys go to it (recipe_key).
fn naturescompass_focused(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<bool> {
    if game.naturescompass_typing() {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
