fn ae2_origin(
    game: &crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<(f32, f32)> {
    match game.ae2_origin(batch) {
        Some(o) => ::core::ops::ControlFlow::Break(o),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
