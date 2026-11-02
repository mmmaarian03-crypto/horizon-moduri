// The guests' tick on a server: the held anvils, then the anvils to the clients.
fn easyanvils_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.easyanvils_guests_tick(host);
}
