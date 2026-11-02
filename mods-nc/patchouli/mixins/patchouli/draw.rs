// The open book takes the place of the Advancements screen.
fn patchouli_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<()> {
    if game.patchouli_draw(batch) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
