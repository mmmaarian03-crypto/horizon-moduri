// The wheel scrolls the hat grid.
fn hats_menu_scroll(game: &mut crate::game::Game<'_>, y: f32) -> ::core::ops::ControlFlow<()> {
    if game.hats_scroll(y) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
