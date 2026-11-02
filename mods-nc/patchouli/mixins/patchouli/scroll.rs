// The wheel turns the pages (advancements_scroll(x, y): y is the wheel's delta).
fn patchouli_scroll(game: &mut crate::game::Game<'_>, y: f32) -> ::core::ops::ControlFlow<()> {
    if game.patchouli_scroll(y) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
