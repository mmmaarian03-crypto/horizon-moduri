// CapsuleItem.use (a right click in the air, or on a block after the block's own use): activate, throw, undeploy,
// or act at once for an instant capsule.
fn capsule_use(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) -> core::ops::ControlFlow<Result<bool>> {
    match game.capsule_use(stack) {
        Some(swing) => core::ops::ControlFlow::Break(Ok(swing)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
