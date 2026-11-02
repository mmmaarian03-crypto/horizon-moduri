fn ae2_key(game: &mut crate::game::Game<'_>, key: winit::keyboard::KeyCode) -> ::core::ops::ControlFlow<bool> {
    match game.ae2_key(key) {
        Some(v) => ::core::ops::ControlFlow::Break(v),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
