fn enderstorage_craft(
    game: &crate::game::Game<'_>,
) -> ::core::ops::ControlFlow<Option<(String, ItemStack)>> {
    // CreateRecipe and ReColourRecipe first, then the game's own recipes.
    match game.enderstorage_craft() {
        Some(found) => ::core::ops::ControlFlow::Break(Some(found)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
