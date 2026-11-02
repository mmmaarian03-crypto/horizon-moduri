fn storagedrawers_take_one(
    game: &crate::game::Game<'_>,
    view: &View,
    slot: usize,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    // The upgrades of a drawer are not for hoppers.
    let Some((pos, _)) = view.locate(slot) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if game.storagedrawers_protects(pos, slot) {
        ::core::ops::ControlFlow::Break(None)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
