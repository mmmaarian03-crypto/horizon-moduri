fn ae2_scroll(
    game: &mut crate::game::Game<'_>,
    delta: f32,
    slot: Option<usize>,
) -> core::ops::ControlFlow<bool> {
    match game.ae2_scroll(delta, slot) {
        Some(v) => core::ops::ControlFlow::Break(v),
        None => core::ops::ControlFlow::Continue(()),
    }
}
