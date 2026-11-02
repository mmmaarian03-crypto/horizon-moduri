// The world's tick: mob hats published, thrown hats moved.
fn hats_tick(game: &mut crate::game::Game<'_>) {
    game.hats_world_tick();
}
