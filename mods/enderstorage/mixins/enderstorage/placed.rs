fn enderstorage_placed(
    game: &mut crate::game::Game<'_>,
    name: &str,
    pos: [i32; 3],
    stack: &horizon_client::items::ItemStack,
) {
    game.enderstorage_placed(name, pos, stack);
}
