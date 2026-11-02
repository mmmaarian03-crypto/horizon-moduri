fn placed(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: BlockStateId, stack: &ItemStack) {
    game.comforts_placed(pos, state, stack);
}
