// Every tick_lan pass, the hub in hand: foundry's frames from the guests, then their open menus.
fn foundry_server_net(game: &mut crate::game::Game<'_>, host: &mut crate::game::lan::LanHost) {
    game.foundry_net_server(host);
}
