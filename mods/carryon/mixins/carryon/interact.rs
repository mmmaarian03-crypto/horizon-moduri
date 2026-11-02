// A right click on a mob: it is picked up, or the carried one is stacked on it.
fn carryon_interact(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Result<bool>> {
    if game.carryon_interact() {
        core::ops::ControlFlow::Break(Ok(true))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
