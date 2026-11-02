fn morebows_use_tick(game: &mut crate::game::Game<'_>, slot: usize, ticks: u32, stack: &ItemStack) -> core::ops::ControlFlow<bool> {
    if game.morebows_tick_use(slot, ticks, stack) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
