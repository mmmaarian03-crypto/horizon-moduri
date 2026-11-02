// Every tick_lan pass, the hub in hand: forja's frames from the guests, then their open menus.
fn forja_server_net(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.forja_net_server(host);
}
