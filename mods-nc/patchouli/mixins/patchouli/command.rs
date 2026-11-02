// /open-patchouli-book <book> [<entry> [<page>]] and /patchouli list | give | open.
fn patchouli_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if game.patchouli_command(command) {
        *ran = true;
    }
}
