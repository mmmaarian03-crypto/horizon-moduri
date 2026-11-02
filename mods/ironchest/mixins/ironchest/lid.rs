fn ironchest_lid(
    game: &mut crate::game::Game<'_>,
    key: (i32, i32, i32),
    open: bool,
) -> ::core::ops::ControlFlow<bool> {
    match game.ironchest_lid(key, open) {
        Some(edge) => ::core::ops::ControlFlow::Break(edge),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
