// The pointer moved with the list open: nothing to scroll the advancements by.
fn naturescompass_drag(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.naturescompass_drag() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
