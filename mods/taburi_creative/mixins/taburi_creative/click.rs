fn taburi_creative_click(game: &mut crate::game::Game<'_>, right: bool) -> ::core::ops::ControlFlow<()> {
    if game.taburi_creative_click(right) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
