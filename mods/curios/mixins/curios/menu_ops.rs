fn curios_menu_ops(
    game: &mut crate::game::Game<'_>,
    ops: &[(Option<usize>, i32, horizon_client::menu::Input)],
) -> ::core::ops::ControlFlow<bool> {
    match game.curios_menu_ops(ops) {
        Some(handled) => ::core::ops::ControlFlow::Break(handled),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
