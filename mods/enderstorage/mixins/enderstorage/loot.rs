fn enderstorage_loot(
    value: Vec<ItemStack>,
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: BlockStateId,
) -> Vec<ItemStack> {
    game.enderstorage_loot(pos, state, value)
}
