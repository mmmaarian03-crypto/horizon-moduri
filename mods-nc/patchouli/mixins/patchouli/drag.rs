// The pointer moved with the book open: nothing to scroll the advancements by.
fn patchouli_drag(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<()> {
    if game.patchouli_release() {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
