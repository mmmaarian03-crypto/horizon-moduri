// The menu engine's crafting result (`CraftingMenu.slotChangedCraftingGrid`): the player's choice, when it
// matches; Break skips the original, which would put the first match there. Otherwise the original runs.
fn polymorph_recompute(
    menu: &mut crate::menu::Menu,
    env: &crate::menu::Env<'_>,
) -> ::core::ops::ControlFlow<()> {
    if crate::menu::polymorph::apply(menu, env) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
