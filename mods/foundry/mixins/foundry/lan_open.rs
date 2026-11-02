// After a guest's turn opened a container: a foundry menu instead of a vanilla one.
fn foundry_lan_open(
    game: &mut crate::game::Game<'_>,
    host: &mut crate::game::lan::LanHost,
    conn: u32,
    pos: (i32, i32, i32),
) -> core::ops::ControlFlow<bool> {
    match game.foundry_lan_open(host, conn, pos) {
        Some(v) => core::ops::ControlFlow::Break(v),
        None => core::ops::ControlFlow::Continue(()),
    }
}
