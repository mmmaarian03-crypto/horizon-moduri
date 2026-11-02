fn ironchest_ticker(state: u32) -> ::core::ops::ControlFlow<Option<Ticker>> {
    // ChestLidController: the chests' lids open and close as a chest's.
    if crate::ironchest::chest_state(state).is_some() {
        ::core::ops::ControlFlow::Break(Some(Ticker::Lid))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
