fn creative_tabs_click(game: &mut crate::game::Game<'_>, right: bool) -> ::core::ops::ControlFlow<()> {
    if game.creative_tabs_click(right) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
