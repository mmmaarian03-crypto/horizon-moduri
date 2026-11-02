fn leavesbegone(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Result<()>> {
    core::ops::ControlFlow::Break(game.leavesbegone_tick_scheduled())
}
