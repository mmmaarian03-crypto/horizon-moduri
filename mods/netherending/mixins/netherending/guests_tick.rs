// The guests' tick on a server: the mobs a guest's break angered see that guest.
fn netherending_guests_tick(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.netherending_guests_tick(host);
}
