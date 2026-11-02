// Once per game tick: the slowness of the load, the slot kept, the carrier's death.
fn carryon_tick(game: &mut crate::game::Game<'_>) {
    game.carryon_tick();
}
