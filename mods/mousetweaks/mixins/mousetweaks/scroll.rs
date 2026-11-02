// The wheel over a slot: Mouse Tweaks takes it whenever it is over a usable slot.
fn mousetweaks_scroll(
    game: &mut crate::game::Game<'_>,
    delta: f32,
    slot: Option<usize>,
) -> core::ops::ControlFlow<bool> {
    if !game.native_on(crate::game::mousetweaks::ID) {
        return core::ops::ControlFlow::Continue(());
    }
    core::ops::ControlFlow::Break(game.mousetweaks_scroll(delta, slot))
}
