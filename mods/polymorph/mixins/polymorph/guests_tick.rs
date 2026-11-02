// The guests' tick on a server: each guest is told his remembered choice once.
fn polymorph_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.polymorph_guests_tick(host);
}
