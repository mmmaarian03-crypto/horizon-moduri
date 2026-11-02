fn enderstorage_open(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    name: &str,
) -> ::core::ops::ControlFlow<bool> {
    if game.enderstorage_open(pos, name) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
