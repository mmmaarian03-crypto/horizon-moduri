fn curios_drag_accepts(
    game: &mut crate::game::Game<'_>,
    slot: usize,
    ty: i32,
    dragged: usize,
) -> ::core::ops::ControlFlow<bool> {
    let _ = (ty, dragged);
    match game.curios_drag_accepts(slot) {
        Some(accepts) => ::core::ops::ControlFlow::Break(accepts),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
