// A guest leaves the server: a held table keeps its grid.
fn visualworkbench_guest_left(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, g: &horizon_client::lan::guest::Guest) {
    game.visualworkbench_guest_closed(host, g.conn, None);
}
