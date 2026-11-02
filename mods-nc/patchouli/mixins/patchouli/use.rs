// A guide book used on the window of a game with a server: the book opens and the click ends (ItemModBook.use answers SUCCESS).
fn patchouli_use(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> ::core::ops::ControlFlow<bool> {
    if game.patchouli_use_item(stack) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
