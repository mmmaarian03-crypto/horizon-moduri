// The crafting screen is drawn: Polymorph's selector button and popup go over it.
fn polymorph_draw(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    game.polymorph_draw(batch);
}
