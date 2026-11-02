fn easyanvils_opened(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    name: &str,
    value: bool,
) -> bool {
    game.easyanvils_opened(pos, name, value);
    value
}
