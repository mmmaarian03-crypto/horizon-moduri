fn try_sleep(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<()> {
    if game.comforts_try_sleep(pos) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
