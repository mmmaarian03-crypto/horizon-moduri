// The pointer moved with the list open: nothing to scroll the advancements by.
fn explorerscompass_drag(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.explorerscompass_drag() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
