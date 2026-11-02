fn curios_mouse(game: &mut crate::game::Game<'_>, button: winit::event::MouseButton) -> ::core::ops::ControlFlow<bool> {
    if game.curios_mouse(button) { ::core::ops::ControlFlow::Break(true) } else { ::core::ops::ControlFlow::Continue(()) }
}
