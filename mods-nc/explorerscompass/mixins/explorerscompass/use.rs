// An item used in the air or on a block (after the block's own use): the compass opens its list, or stops its searches when
// sneaking (`ExplorersCompassItem.use`, which answers PASS: the click is not consumed).
fn explorerscompass_use(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> core::ops::ControlFlow<Result<bool>> {
    if game.explorerscompass_use(stack) {
        core::ops::ControlFlow::Break(Ok(false))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
