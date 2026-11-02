// CommonEventBusSubscriber.playerLeftClick: a creative left click on a drawer's front takes an
// item instead of breaking the drawer.
fn storagedrawers_mining(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<Result<()>> {
    if game.storagedrawers_creative_mining() {
        core::ops::ControlFlow::Break(Ok(()))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
