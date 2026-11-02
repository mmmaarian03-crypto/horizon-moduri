// A guest's click on his inventory menu (2x2 grid) on a server: the engine reads his choice.
fn polymorph_guest_inventory(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32) {
    game.polymorph_guest_op(host, conn, false);
}
