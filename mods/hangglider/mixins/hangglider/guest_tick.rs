// The server's tick of a guest's body (before its moves are checked for fall damage).
fn hangglider_guest_tick(game: &mut crate::game::Game<'_>, host: &mut LanHost, conn: u32, fell: f64) {
    game.hangglider_guest_tick(host, conn, fell);
}
