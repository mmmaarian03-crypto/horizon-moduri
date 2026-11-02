fn enderstorage_title(game: &crate::game::Game<'_>) -> ::core::ops::ControlFlow<String> {
    match game.enderstorage_title() {
        Some(title) => ::core::ops::ControlFlow::Break(title),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
