// A hopper under an infuser: canTakeItemThroughFace (only an enchanted item or an enchanted book).
fn enchantinginfuser_take_one(
    game: &crate::game::Game<'_>,
    view: &View,
    slot: usize,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    let Some(((x, y, z), _)) = view.locate(slot) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if game.infuser_protects([x, y, z]) {
        ::core::ops::ControlFlow::Break(None)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
