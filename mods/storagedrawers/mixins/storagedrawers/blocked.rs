fn storagedrawers_blocked(name: &str) -> ::core::ops::ControlFlow<bool> {
    // Whatever click the barrel the drawer copies would turn into a menu: no menu, the use
    // still counts (the drawer's own clicks were handled before, in storagedrawers_use).
    if crate::game::storagedrawers::storagedrawers_size(name).is_some() {
        ::core::ops::ControlFlow::Break(true)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
