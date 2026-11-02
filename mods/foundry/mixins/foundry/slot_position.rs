fn foundry_slot_position(
    game: &crate::game::Game<'_>,
    i: usize,
) -> ::core::ops::ControlFlow<Option<(f32, f32)>> {
    match game.foundry_slot_position(i) {
        Some(position) => ::core::ops::ControlFlow::Break(position),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
