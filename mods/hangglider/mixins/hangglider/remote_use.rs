// GliderItem.use on the window's client (MultiPlayerGameMode.useItem), before its packet goes: the client's
// copy of the click.
fn hangglider_remote_use(game: &mut crate::game::Game<'_>, hand: Hand) {
    game.hangglider_remote_use(hand);
}
