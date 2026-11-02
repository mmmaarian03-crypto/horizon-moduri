// A left press over Polymorph's selector button / popup: ours, not a slot click (Break(true) = consumed).
fn polymorph_press(
    game: &mut crate::game::Game<'_>,
    button: winit::event::MouseButton,
) -> ::core::ops::ControlFlow<bool> {
    if game.polymorph_press(button) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
