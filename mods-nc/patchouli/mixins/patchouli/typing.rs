// A key typed into the search bar of a list.
fn patchouli_typing(
    game: &mut crate::game::Game<'_>,
    event: &winit::event::KeyEvent,
) -> ::core::ops::ControlFlow<()> {
    if game.patchouli_type(event) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
