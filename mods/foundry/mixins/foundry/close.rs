// A guest menu closing here closes on the server too.
fn foundry_close(game: &mut crate::game::Game<'_>) {
    game.foundry_guest_close();
}
