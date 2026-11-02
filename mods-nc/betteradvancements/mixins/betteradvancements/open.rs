// The advancements screen opens.
fn betteradvancements_open(game: &mut crate::game::Game<'_>) {
    if game.native_on(crate::game::betteradvancements::ID) {
        game.betteradvancements_open();
    }
}
