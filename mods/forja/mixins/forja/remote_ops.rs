// The game's own remote clicks (mouse tweaks, sorting) on a guest menu go to the server as forja input.
fn forja_remote_ops(
    game: &mut crate::game::Game<'_>,
    ops: &[(Option<usize>, i32, Input)],
) -> ::core::ops::ControlFlow<bool> {
    match game.forja_guest_menu_ops(ops) {
        Some(v) => ::core::ops::ControlFlow::Break(v),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
