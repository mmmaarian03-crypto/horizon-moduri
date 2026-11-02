// A guest menu closing here closes on the server too.
fn forja_close(game: &mut crate::game::Game<'_>) {
    game.forja_guest_close();
}
