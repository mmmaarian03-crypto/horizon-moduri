// A key press while the book is open (Escape closes it, Backspace goes back).
fn patchouli_key(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
) -> ::core::ops::ControlFlow<bool> {
    if game.patchouli_key(key) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
