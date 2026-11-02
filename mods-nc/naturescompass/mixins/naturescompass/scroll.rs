// The wheel over the list (`advancements_scroll(x, y)`: `y` is the wheel's delta).
fn naturescompass_scroll(game: &mut crate::game::Game<'_>, y: f32) -> core::ops::ControlFlow<()> {
    if game.naturescompass_scroll(y) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
