// Mining a compound block: its targeted part (or every part, for hosts that break whole).
fn foundry_break_part(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> ::core::ops::ControlFlow<Result<()>> {
    match game.foundry_break_part(pos) {
        Some(done) => ::core::ops::ControlFlow::Break(done),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
