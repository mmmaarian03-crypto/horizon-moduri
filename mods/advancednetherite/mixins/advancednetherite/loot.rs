fn advancednetherite_loot(
    value: Vec<ItemStack>,
    game: &mut crate::game::Game<'_>,
    state: BlockStateId,
    held: Option<&ItemStack>,
) -> Vec<ItemStack> {
    game.advancednetherite_block_loot(state, held, value)
}
