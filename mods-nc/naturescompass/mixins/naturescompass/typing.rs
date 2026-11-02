// A key typed while the list's search field has the focus.
fn naturescompass_typing(
    game: &mut crate::game::Game<'_>,
    event: &winit::event::KeyEvent,
) -> ::core::ops::ControlFlow<()> {
    if game.naturescompass_type(event) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
