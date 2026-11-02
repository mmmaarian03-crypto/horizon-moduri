fn fallingtree(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<Result<()>> {
    match game.fallingtree_break(pos) {
        Some(felled) => core::ops::ControlFlow::Break(felled),
        None => core::ops::ControlFlow::Continue(()),
    }
}
