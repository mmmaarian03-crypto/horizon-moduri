// A guest menu's clicks go to the server (before every mod's own menu hook).
fn foundry_menu_ops(
    game: &mut crate::game::Game<'_>,
    ops: &[(Option<usize>, i32, horizon_client::menu::Input)],
) -> ::core::ops::ControlFlow<bool> {
    match game.foundry_guest_menu_ops(ops) {
        Some(v) => ::core::ops::ControlFlow::Break(v),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
