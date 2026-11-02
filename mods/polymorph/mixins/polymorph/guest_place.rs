// A guest's recipe book places a recipe on a server: the engine reads his choice.
fn polymorph_guest_place(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32) {
    game.polymorph_guest_op(host, conn, false);
}
