fn clumps_award(game: &mut crate::game::Game<'_>, at: [f64; 3], value: u32) -> core::ops::ControlFlow<()> {
    if game.clumps_award(at, value) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
