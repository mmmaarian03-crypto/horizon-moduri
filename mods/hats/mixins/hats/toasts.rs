// The HUD's last layer: the New Hat Unlocked toasts.
fn hats_toasts(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    game.hats_draw_toasts(batch);
}
