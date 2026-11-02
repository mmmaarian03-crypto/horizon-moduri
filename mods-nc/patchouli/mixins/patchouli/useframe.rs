// The same when the world runs in the frame (--in-frame).
fn patchouli_useframe(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> ::core::ops::ControlFlow<bool> {
    if game.patchouli_use_item(stack) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
