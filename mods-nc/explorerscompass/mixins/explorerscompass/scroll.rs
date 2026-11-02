// The wheel over the list (`advancements_scroll(x, y)`: `y` is the wheel's delta).
fn explorerscompass_scroll(game: &mut crate::game::Game<'_>, y: f32) -> core::ops::ControlFlow<()> {
    if game.explorerscompass_scroll(y) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
