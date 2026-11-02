// A drag step with the left button.
fn betteradvancements_drag(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<()> {
    if !game.native_on(crate::game::betteradvancements::ID) {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.betteradvancements_drag();
    ::core::ops::ControlFlow::Break(())
}
