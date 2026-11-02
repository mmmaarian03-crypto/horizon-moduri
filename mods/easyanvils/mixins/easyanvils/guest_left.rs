// A guest leaves the server: a held anvil keeps its items.
fn easyanvils_guest_left(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, g: &horizon_client::lan::guest::Guest) {
    game.easyanvils_guest_closed(host, g.conn, None);
}
