fn curios_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran {
        game.curios_native_command(command, ran);
    }
}
