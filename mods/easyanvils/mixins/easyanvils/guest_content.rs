// A guest's station menu opened or changed on a server: an anvil's items.
fn easyanvils_guest_content(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost, conn: u32) {
    game.easyanvils_guest_content(host, conn);
}
