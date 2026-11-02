// The release of the left button over the open book (its press went through sort_mouse_press).
fn patchouli_mouse(
    game: &mut crate::game::Game<'_>,
    pressed: bool,
    size: [u32; 2],
) -> ::core::ops::ControlFlow<()> {
    let _ = (pressed, size);
    if game.patchouli_release() {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
