// Every Game's tick: a server's client takes its server's anvils.
fn easyanvils_client_tick(game: &mut crate::game::Game<'_>) {
    game.easyanvils_client_tick();
}
