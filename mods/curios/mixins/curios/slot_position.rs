fn curios_slot_position(game: &crate::game::Game<'_>, i: usize) -> ::core::ops::ControlFlow<Option<(f32, f32)>> {
    match game.curios_slot_position(i) {
        Some(p) => ::core::ops::ControlFlow::Break(p),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
