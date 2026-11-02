// The wheel (16 pixels a step, as `mouseScrolled`).
fn betteradvancements_scroll(game: &mut crate::game::Game<'_>, x: f32, y: f32) -> ::core::ops::ControlFlow<()> {
    if !game.native_on(crate::game::betteradvancements::ID) {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.betteradvancements_scroll_by(f64::from(x) * 16.0, f64::from(y) * 16.0);
    ::core::ops::ControlFlow::Break(())
}
