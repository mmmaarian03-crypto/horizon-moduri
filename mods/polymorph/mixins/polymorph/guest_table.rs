// A guest's click on his crafting table on a server: his choice, the result made again first.
fn polymorph_guest_table(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32) {
    game.polymorph_guest_op(host, conn, true);
}
