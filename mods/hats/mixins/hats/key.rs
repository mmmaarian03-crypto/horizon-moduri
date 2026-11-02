// A key: H opens the hat menu.
fn hats_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    game.hats_key(key, pressed);
}
