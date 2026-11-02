// A guest's crafting menu closes on a server: a held table keeps its grid.
fn visualworkbench_guest_closed(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32, window: Option<i32>) {
    game.visualworkbench_guest_closed(host, conn, window);
}
