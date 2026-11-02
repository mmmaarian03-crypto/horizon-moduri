// Before the moved player's tick: the pose code reads whether he glides.
fn hangglider_before_move(game: &mut crate::game::Game<'_>) {
    game.hangglider_before_move();
}
