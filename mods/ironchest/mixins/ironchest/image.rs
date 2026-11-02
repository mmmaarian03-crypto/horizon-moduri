fn ironchest_image(
    game: &crate::game::Game<'_>,
    batch: &horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<horizon_client::container_screen::Image> {
    match game.ironchest_image(batch) {
        Some(image) => ::core::ops::ControlFlow::Break(image),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
