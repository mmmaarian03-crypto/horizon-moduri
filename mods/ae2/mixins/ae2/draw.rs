fn ae2_draw(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    ox: f32,
    oy: f32,
) -> ::core::ops::ControlFlow<bool> {
    if game.ae2_draw(batch, ox, oy) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
