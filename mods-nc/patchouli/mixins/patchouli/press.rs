// A press of a mouse button while the book is open (Break(true) = consumed).
fn patchouli_press(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
) -> ::core::ops::ControlFlow<bool> {
    if game.patchouli_press(button) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
