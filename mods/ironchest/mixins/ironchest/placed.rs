fn ironchest_placed(
    game: &mut crate::game::Game<'_>,
    name: &str,
    pos: [i32; 3],
    stack: &horizon_client::items::ItemStack,
) {
    game.ironchest_placed(name, pos, stack);
}
