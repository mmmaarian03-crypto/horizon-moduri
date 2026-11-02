// The compass' list takes the place of the Advancements screen while it is open.
fn explorerscompass_draw(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) -> core::ops::ControlFlow<()> {
    if game.explorerscompass_draw(batch) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
