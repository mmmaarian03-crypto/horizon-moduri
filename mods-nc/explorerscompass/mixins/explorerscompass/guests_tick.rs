// The guests' tick on the server: each guest's structure search, teleport and compasses, as the guest.
fn explorerscompass_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.explorerscompass_guests_tick(host);
}
