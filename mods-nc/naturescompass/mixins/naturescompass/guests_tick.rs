// The guests' tick on the server: each guest's biome search, teleport and compasses, as the guest.
fn naturescompass_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.naturescompass_guests_tick(host);
}
