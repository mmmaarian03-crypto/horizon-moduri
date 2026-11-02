// HEAD on `guest_glide_tick`: no fall distance for a guest whose jetpack pushed this tick.
fn simplyjetpacks_guest_fall(game: &mut crate::game::Game<'_>, host: &mut LanHost, conn: u32, fell: f64) {
    let _ = fell;
    game.simplyjetpacks_guest_fall(host, conn);
}
