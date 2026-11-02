// A guest's station menu closes on a server: a held anvil keeps its items.
fn easyanvils_guest_closed(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32, window: Option<i32>) {
    game.easyanvils_guest_closed(host, conn, window);
}
