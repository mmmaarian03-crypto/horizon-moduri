// A left click: no attack while carrying (`hitWhileCarrying` is off).
fn carryon_attack(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    if game.carryon_no_hit() {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
