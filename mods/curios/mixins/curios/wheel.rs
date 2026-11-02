fn curios_wheel(game: &mut crate::game::Game<'_>, delta: f32) -> ::core::ops::ControlFlow<bool> {
    if game.curios_wheel(delta) { ::core::ops::ControlFlow::Break(true) } else { ::core::ops::ControlFlow::Continue(()) }
}
