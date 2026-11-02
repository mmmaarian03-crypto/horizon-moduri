// The guests' tick on the server: each guest's last place under the sky.
fn escape_rope_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.escape_rope_guests_tick(host);
}
