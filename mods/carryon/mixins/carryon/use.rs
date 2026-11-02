// A right click on a block (hands empty also when sneaking): the carried object goes down, or the block is picked up.
fn carryon_use(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<bool> {
    if game.carryon_use_block(pos) {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
