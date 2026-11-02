// Every Game's tick: a server's client takes the choice its server remembers for it.
fn polymorph_client_tick(game: &mut crate::game::Game<'_>) {
    game.polymorph_client_tick();
}
