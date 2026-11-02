fn use_on(game: &mut crate::game::Game<'_>, stack: &ItemStack) -> core::ops::ControlFlow<Result<bool>> {
    match game.comforts_use_on(stack) {
        Some(result) => core::ops::ControlFlow::Break(result),
        None => core::ops::ControlFlow::Continue(()),
    }
}
