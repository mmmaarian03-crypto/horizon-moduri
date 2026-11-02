fn tick_sleep(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if game.comforts_tick_sleep() {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
