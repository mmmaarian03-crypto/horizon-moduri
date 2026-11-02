// The player placed a block.
fn a_good_place_place(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: BlockStateId, stack: &ItemStack) {
    game.a_good_place_placed(pos, state, stack);
}
