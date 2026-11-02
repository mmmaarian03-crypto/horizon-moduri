// Each tick: thrown capsules, the in-frame host's capsules and starter capsule.
fn capsule_tick(game: &mut crate::game::Game<'_>) {
    game.capsule_tick();
}
