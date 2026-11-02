// The cursor moved: the drag tweaks (the rules only act while a button is down).
fn mousetweaks_drag(game: &mut crate::game::Game<'_>) {
    game.mousetweaks_moved();
}
