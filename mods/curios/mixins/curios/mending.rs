fn curios_mending(game: &mut crate::game::Game<'_>, amount: u32) -> ::core::ops::ControlFlow<u32> {
    match game.curios_mending(amount) {
        Some(rest) => ::core::ops::ControlFlow::Break(rest),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
