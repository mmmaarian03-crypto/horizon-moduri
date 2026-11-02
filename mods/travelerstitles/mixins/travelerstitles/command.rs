// A command typed in chat: Traveler's Titles' three.
fn travelerstitles_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if game.travelerstitles_command(command) {
        *ran = true;
    }
}
