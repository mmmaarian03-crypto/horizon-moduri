// A right click of the window that reached Item.use: pick up a thrown hat, the launcher's screen.
fn hats_use_hands(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<()> {
    if game.hats_window_use() {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
