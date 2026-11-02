// A press or release of the left button on the list.
fn explorerscompass_mouse(game: &mut crate::game::Game<'_>, pressed: bool, size: [u32; 2]) -> core::ops::ControlFlow<()> {
    if game.explorerscompass_mouse(pressed, size) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
