fn storagedrawers_counts(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    // StorageGuiGraphics: the drawer slots' counts, half size, over their items.
    game.storagedrawers_draw_counts(batch);
}
