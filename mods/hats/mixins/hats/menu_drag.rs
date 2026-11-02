// No panning of the advancement tree under the hat menu.
fn hats_menu_drag(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<()> {
    if game.hats_drag() {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
