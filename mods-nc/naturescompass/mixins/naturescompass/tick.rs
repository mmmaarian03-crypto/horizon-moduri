// Once per game tick: the in-frame host's biome search, pending teleport and compasses.
fn naturescompass_tick(game: &mut crate::game::Game<'_>) {
    game.naturescompass_tick();
}
