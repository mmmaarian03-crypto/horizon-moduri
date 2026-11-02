// The Advancements screen draws the hat menu while it is open.
fn hats_menu_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<()> {
    if game.hats_draw_menu(batch) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
