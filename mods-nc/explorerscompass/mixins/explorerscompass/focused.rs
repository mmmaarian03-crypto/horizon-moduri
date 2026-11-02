// While the list's search field has the focus, the window's typed keys go to it (recipe_key).
fn explorerscompass_focused(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<bool> {
    if game.explorerscompass_typing() {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
