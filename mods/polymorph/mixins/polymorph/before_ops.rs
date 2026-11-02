// The window's own menu clicks: the menu engine reads this player's choice.
fn polymorph_before_ops(game: &mut crate::game::Game<'_>) {
    game.polymorph_before_menu_ops();
}
