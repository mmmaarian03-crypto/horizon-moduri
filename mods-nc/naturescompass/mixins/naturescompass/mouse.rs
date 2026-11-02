// A press or release of the left button on the list.
fn naturescompass_mouse(game: &mut crate::game::Game<'_>, pressed: bool, size: [u32; 2]) -> core::ops::ControlFlow<()> {
    if game.naturescompass_mouse(pressed, size) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
