fn ironchest_upgrade(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    held: Option<&horizon_client::items::ItemStack>,
) -> ::core::ops::ControlFlow<bool> {
    // ChestUpgradeItem.onItemUseFirst comes before the block's own use.
    if game.ironchest_upgrade(pos, held) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
