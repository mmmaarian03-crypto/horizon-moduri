fn ironchest_drag_view(
    game: &mut crate::game::Game<'_>,
) -> ::core::ops::ControlFlow<Option<DragView>> {
    match game.ironchest_drag_view() {
        Some(view) => ::core::ops::ControlFlow::Break(view),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
