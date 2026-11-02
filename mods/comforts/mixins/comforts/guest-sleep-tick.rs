fn guest_sleep_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, gi: usize) -> core::ops::ControlFlow<()> {
    if game.comforts_guest_sleep_tick(host, gi) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
