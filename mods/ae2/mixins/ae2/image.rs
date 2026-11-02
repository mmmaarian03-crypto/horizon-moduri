fn ae2_image(
    game: &crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<horizon_client::container_screen::Image> {
    match game.ae2_image(batch) {
        Some(i) => ::core::ops::ControlFlow::Break(i),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
