fn ae2_mouse(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
) -> ::core::ops::ControlFlow<bool> {
    if game.ae2_mouse(button) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
