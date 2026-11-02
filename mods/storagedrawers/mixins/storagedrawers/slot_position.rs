fn storagedrawers_slot_position(
    game: &crate::game::Game<'_>,
    i: usize,
) -> ::core::ops::ControlFlow<Option<(f32, f32)>> {
    // ContainerDrawers' slot coordinates.
    match game.storagedrawers_slot_position(i) {
        Some(p) => ::core::ops::ControlFlow::Break(p),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
