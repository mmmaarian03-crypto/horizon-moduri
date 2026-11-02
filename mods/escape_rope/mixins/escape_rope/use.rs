fn escape_rope_use(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) -> core::ops::ControlFlow<Result<bool>> {
    if &*stack.item == "escape_rope:escape_rope" {
        game.escape_rope_use(stack);
        // Item.use returned PASS: the click is not consumed.
        core::ops::ControlFlow::Break(Ok(false))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
