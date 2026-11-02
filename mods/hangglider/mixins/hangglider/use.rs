// GliderItem.use (a host in the frame, or a guest's use on the server): the glider opens or closes.
fn hangglider_use(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) -> core::ops::ControlFlow<Result<bool>> {
    if game.hangglider_use(stack) {
        core::ops::ControlFlow::Break(Ok(true))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
