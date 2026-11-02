fn ae2_take_one(
    game: &crate::game::Game<'_>,
    view: &View,
    slot: usize,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    // ...nor takes from them.
    match view.locate(slot) {
        Some((pos, _)) if game.ae2_window_at(pos) => ::core::ops::ControlFlow::Break(None),
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
