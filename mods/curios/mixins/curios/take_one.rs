fn curios_take_one(game: &crate::game::Game<'_>, view: &View) -> ::core::ops::ControlFlow<Option<ItemStack>> {
    // Nothing comes out of a Curios menu's mirror.
    let parts: Vec<([i32; 3], usize)> = view.parts.iter().map(|&(p, n)| ([p.0, p.1, p.2], n)).collect();
    if game.curios_mirror_view(&parts) {
        ::core::ops::ControlFlow::Break(None)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
