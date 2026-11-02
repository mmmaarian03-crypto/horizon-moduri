// HopperBlockEntity.tryMoveInItem into an infuser: canPlaceItemThroughFace (only an empty slot).
fn enchantinginfuser_move_in(
    game: &mut crate::game::Game<'_>,
    view: &View,
    stack: &ItemStack,
    slot: usize,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    let Some(((x, y, z), _)) = view.locate(slot) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    match game.infuser_move_in([x, y, z], stack) {
        Some(rest) => ::core::ops::ControlFlow::Break(rest),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
