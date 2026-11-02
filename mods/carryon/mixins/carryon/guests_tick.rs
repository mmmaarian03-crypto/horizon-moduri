// The guests' tick on the server: each guest's carry (slowness, slot, death) and its client's copy.
fn carryon_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.carryon_guests_tick(host);
}
