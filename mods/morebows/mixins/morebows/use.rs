fn morebows_use(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) -> core::ops::ControlFlow<Result<bool>> {
    if game.morebows_use(stack) {
        core::ops::ControlFlow::Break(Ok(true))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
