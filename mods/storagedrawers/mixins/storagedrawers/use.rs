fn storagedrawers_use(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    held: Option<&horizon_client::items::ItemStack>,
) -> ::core::ops::ControlFlow<bool> {
    // BlockDrawers.use / BlockController.use come before anything else on the block.
    if game.storagedrawers_use(pos, held) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
