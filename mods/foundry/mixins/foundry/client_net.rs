// A server's client, every tick: the foundry frames the server sent.
fn foundry_client_net(game: &mut crate::game::Game<'_>) {
    game.foundry_net_client();
}
