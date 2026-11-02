fn enderstorage_pouch(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> ::core::ops::ControlFlow<bool> {
    // ItemEnderPouch.use / onItemUseFirst.
    if game.enderstorage_pouch(stack) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
