fn tia_model(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    stack: &horizon_client::items::ItemStack,
    x: f32,
    y: f32,
) -> core::ops::ControlFlow<()> {
    game.tia_model(batch, stack, x, y)
}
