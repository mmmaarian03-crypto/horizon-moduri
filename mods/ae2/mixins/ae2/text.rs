fn ae2_text(game: &mut crate::game::Game<'_>, text: Option<&str>, backspace: bool) -> ::core::ops::ControlFlow<()> {
    if game.ae2_text(text, backspace) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
