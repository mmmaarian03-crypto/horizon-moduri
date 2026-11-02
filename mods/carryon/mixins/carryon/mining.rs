// The held left button: no mining while carrying (`BreakSpeed` 0 in the original).
fn carryon_mining(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Result<()>> {
    if game.carryon_no_hit() {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
