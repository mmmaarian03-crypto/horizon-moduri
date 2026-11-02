fn storagedrawers_move_in(
    game: &mut crate::game::Game<'_>,
    view: &View,
    stack: &ItemStack,
    slot: usize,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    // HopperBlockEntity.tryMoveInItem into a drawer: the drawer's rules, its upgrade slots shut.
    let Some((pos, _)) = view.locate(slot) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    match game.storagedrawers_move_in(pos, slot, stack) {
        Some(rest) => ::core::ops::ControlFlow::Break(rest),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
