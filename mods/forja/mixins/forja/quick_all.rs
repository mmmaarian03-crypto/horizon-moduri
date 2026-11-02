// A Shift double click on a guest menu: the second shift-click goes to the server (no local guess).
fn forja_quick_all(game: &mut crate::game::Game<'_>, slot: usize) -> ::core::ops::ControlFlow<()> {
    if game.forja_guest_quick_all(slot) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
