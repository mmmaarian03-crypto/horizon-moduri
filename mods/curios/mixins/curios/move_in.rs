fn curios_move_in(
    game: &mut crate::game::Game<'_>,
    view: &View,
    stack: &ItemStack,
) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    // A Curios menu's mirror is no container: nothing goes in.
    let parts: Vec<([i32; 3], usize)> = view.parts.iter().map(|&(p, n)| ([p.0, p.1, p.2], n)).collect();
    if game.curios_mirror_view(&parts) {
        ::core::ops::ControlFlow::Break(Some(stack.clone()))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
