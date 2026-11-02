// Once per game tick: the in-frame host's structure search, pending teleport and compasses.
fn explorerscompass_tick(game: &mut crate::game::Game<'_>) {
    game.explorerscompass_tick();
}
