// The crafting grid's recipe (result slot, shift-click loop, display): the one the player chose, when it
// matches (Break); otherwise the game's own first match.
fn polymorph_craft(
    game: &crate::game::Game<'_>,
) -> ::core::ops::ControlFlow<Option<(String, ItemStack)>> {
    match game.polymorph_craft() {
        Some(chosen) => ::core::ops::ControlFlow::Break(Some(chosen)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
