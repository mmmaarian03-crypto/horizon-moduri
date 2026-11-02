// Every Game's tick: a server's client takes its server's tables and turns their items.
fn visualworkbench_client_tick(game: &mut crate::game::Game<'_>) {
    game.visualworkbench_client_tick();
}
