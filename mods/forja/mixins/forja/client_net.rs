// A server's client, every tick: the forja frames the server sent.
fn forja_client_net(game: &mut crate::game::Game<'_>) {
    game.forja_net_client();
}
