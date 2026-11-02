fn soulshards_use(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<bool> {
    if game.soulshards_use(pos) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
