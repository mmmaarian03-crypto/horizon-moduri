fn ae2_use(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<bool> {
    // AE2's onActivated for its blocks (open a screen, turn a crank, charge an item).
    if game.ae2_use(pos) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
