// A left button press or release on the screen.
fn betteradvancements_mouse(game: &mut crate::game::Game<'_>, pressed: bool, size: [u32; 2]) -> ::core::ops::ControlFlow<()> {
    if !game.native_on(crate::game::betteradvancements::ID) {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.betteradvancements_mouse(pressed, size);
    ::core::ops::ControlFlow::Break(())
}
