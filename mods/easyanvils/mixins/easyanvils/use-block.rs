fn easyanvils_use(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    held: Option<&ItemStack>,
) -> ::core::ops::ControlFlow<bool> {
    if game.easyanvils_use(pos, held) {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
