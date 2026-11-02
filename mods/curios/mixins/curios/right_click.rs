fn curios_right_click(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> ::core::ops::ControlFlow<Result<bool>> {
    match game.curios_right_click(stack) {
        Some(done) => ::core::ops::ControlFlow::Break(Ok(done)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
