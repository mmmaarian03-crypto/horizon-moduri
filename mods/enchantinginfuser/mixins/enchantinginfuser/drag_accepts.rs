fn enchantinginfuser_drag_accepts(
    game: &mut crate::game::Game<'_>,
    slot: usize,
    ty: i32,
    dragged: usize,
) -> ::core::ops::ControlFlow<bool> {
    match game.infuser_quick_craft_accepts(slot, ty, dragged) {
        Some(accepts) => ::core::ops::ControlFlow::Break(accepts),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
