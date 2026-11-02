// The infuser's widgets take the press before the slots (every press stops here first).
fn enchantinginfuser_mouse(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
) -> ::core::ops::ControlFlow<bool> {
    if game.infuser_mouse(button) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
