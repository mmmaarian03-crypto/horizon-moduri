fn curios_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    game.curios_key_event(key, pressed);
}
