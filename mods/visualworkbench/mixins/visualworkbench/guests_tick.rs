// The guests' tick on a server: the held tables, then the tables to the clients.
fn visualworkbench_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.visualworkbench_guests_tick(host);
}
