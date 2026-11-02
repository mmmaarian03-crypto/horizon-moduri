// The screen, drawn by the mod while it is on.
fn betteradvancements_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<()> {
    if !game.native_on(crate::game::betteradvancements::ID) {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.betteradvancements_draw(batch);
    ::core::ops::ControlFlow::Break(())
}
