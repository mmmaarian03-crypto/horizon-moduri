fn torchmaster_pearl(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> ::core::ops::ControlFlow<Result<bool>> {
    match game.torchmaster_pearl(stack) {
        Some(used) => ::core::ops::ControlFlow::Break(used),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
