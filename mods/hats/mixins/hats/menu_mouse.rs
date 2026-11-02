// Clicks on the hat menu.
fn hats_menu_mouse(
    game: &mut crate::game::Game<'_>,
    pressed: bool,
    size: [u32; 2],
) -> ::core::ops::ControlFlow<()> {
    if game.hats_mouse(pressed, size) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
