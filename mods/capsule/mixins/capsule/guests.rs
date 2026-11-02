// The guests' tick on the server (single player's window included): their capsules' timers and starter capsule.
fn capsule_guests(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.capsule_guests_tick(host);
}
