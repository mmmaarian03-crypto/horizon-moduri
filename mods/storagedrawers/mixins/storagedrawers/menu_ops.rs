fn storagedrawers_menu_ops(
    game: &mut crate::game::Game<'_>,
    ops: &[(Option<usize>, i32, horizon_client::menu::Input)],
) -> ::core::ops::ControlFlow<bool> {
    // ContainerDrawers' clicks on the server (or the in-frame host).
    match game.storagedrawers_menu_ops(ops) {
        Some(handled) => ::core::ops::ControlFlow::Break(handled),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
