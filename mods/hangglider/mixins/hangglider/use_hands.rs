// Minecraft.startUseItem on the window's client: a glider in the main hand that opens / closes is a
// SUCCESS (sidedSuccess): the main hand's use packet, the swing and itemUsed, and the off hand gets no turn
// (remote_use_item cannot answer `true` for it: its RETURN is refused). This helper lives in remote.rs, so
// it may call the game's own private remote_use_item.
fn hangglider_use_hands(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<()> {
    if !game.hangglider_main_toggles() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let _ = game.remote_use_item(Hand::Main);
    game.with_hand(Hand::Main, crate::game::Game::swing_arm);
    game.heights.item_used(Hand::Main);
    ::core::ops::ControlFlow::Break(())
}
