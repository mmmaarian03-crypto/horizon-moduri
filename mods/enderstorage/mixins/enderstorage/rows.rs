fn enderstorage_rows(
    game: &crate::game::Game<'_>,
) -> ::core::ops::ControlFlow<Option<std::sync::Arc<str>>> {
    match game.enderstorage_container_block() {
        Some(name) => ::core::ops::ControlFlow::Break(Some(name)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
