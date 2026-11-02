// A guest's crafting table menu opened or changed on a server: the table's grid.
fn visualworkbench_guest_content(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32) {
    game.visualworkbench_guest_content(host, conn);
}
