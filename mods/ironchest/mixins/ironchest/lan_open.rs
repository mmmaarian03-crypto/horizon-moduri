fn ironchest_lan_open(
    game: &mut crate::game::Game<'_>,
    host: &mut crate::game::lan::LanHost,
    conn: u32,
    pos: (i32, i32, i32),
) -> ::core::ops::ControlFlow<bool> {
    match game.ironchest_lan_open(host, conn, pos) {
        Some(opened) => ::core::ops::ControlFlow::Break(opened),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
