fn enderstorage_use(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    held: Option<&horizon_client::items::ItemStack>,
) -> ::core::ops::ControlFlow<bool> {
    // BlockEnderStorage.use: the buttons, the latch and the tank's bucket, before the block's own use.
    if game.enderstorage_use(pos, held) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
