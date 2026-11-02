fn ironchest_origin(
    game: &crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<(f32, f32)> {
    match game.ironchest_origin(batch) {
        Some(origin) => ::core::ops::ControlFlow::Break(origin),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
