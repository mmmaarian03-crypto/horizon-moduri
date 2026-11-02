// A key press while the list is open (Break(true) = consumed; Escape closes the list).
fn explorerscompass_key(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
) -> ::core::ops::ControlFlow<bool> {
    if game.explorerscompass_key(key) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
