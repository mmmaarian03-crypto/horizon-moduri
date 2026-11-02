// A key press while the list is open (Break(true) = consumed; Escape closes the list).
fn naturescompass_key(
    game: &mut crate::game::Game<'_>,
    key: winit::keyboard::KeyCode,
) -> ::core::ops::ControlFlow<bool> {
    if game.naturescompass_key(key) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
