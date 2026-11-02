// Item.use of the Hat Launcher.
fn hats_launch(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> core::ops::ControlFlow<Result<bool>> {
    match game.hats_launch(stack) {
        Some(swing) => core::ops::ControlFlow::Break(Ok(swing)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
