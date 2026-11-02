fn ae2_move_in(
    game: &mut crate::game::Game<'_>,
    view: &View,
    stack: &ItemStack,
    slot: usize,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    // A hopper never fills an AE2 window's temporary slots.
    match view.locate(slot) {
        Some((pos, _)) if game.ae2_window_at(pos) => ::core::ops::ControlFlow::Break(Some(stack.clone())),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
