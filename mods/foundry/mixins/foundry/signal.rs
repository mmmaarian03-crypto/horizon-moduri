// The signal of a block that emits through foundry (set_redstone); others: the game's answer.
fn foundry_signal(level: &GameLevel<'_>, pos: BlockPos, direction: Direction) -> ::core::ops::ControlFlow<i32> {
    match crate::game::foundry::redstone_answer(level.dimension, [pos.x, pos.y, pos.z], direction) {
        Some(v) => ::core::ops::ControlFlow::Break(v),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
