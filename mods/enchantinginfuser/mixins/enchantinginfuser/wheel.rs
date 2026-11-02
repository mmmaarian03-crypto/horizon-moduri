// InfuserScreen.mouseScrolled: the wheel scrolls the list.
fn enchantinginfuser_wheel(
    game: &mut crate::game::Game<'_>,
    delta: f32,
) -> core::ops::ControlFlow<bool> {
    if game.infuser_wheel(delta) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
