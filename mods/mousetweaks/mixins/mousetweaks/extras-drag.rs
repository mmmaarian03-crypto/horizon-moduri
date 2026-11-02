// With Mouse Tweaks on, Horizon's extras drag does not run; vanilla's quick craft drag still does.
fn mousetweaks_extras_drag(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<bool> {
    if game.native_on(crate::game::mousetweaks::ID) {
        core::ops::ControlFlow::Break(false)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
