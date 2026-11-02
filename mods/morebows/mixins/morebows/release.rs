fn morebows_release(game: &mut crate::game::Game<'_>, stack: &ItemStack, ticks: u32, hand_changed: bool) -> core::ops::ControlFlow<()> {
    if game.morebows_release(stack, ticks, hand_changed) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
