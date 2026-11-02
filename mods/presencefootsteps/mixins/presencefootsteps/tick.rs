// The footsteps of the entities near the camera, once a tick.
fn presencefootsteps_tick(game: &mut crate::game::Game<'_>) {
    game.pf_tick();
}
