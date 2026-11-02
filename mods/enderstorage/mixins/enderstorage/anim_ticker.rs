fn enderstorage_ticker(state: u32) -> ::core::ops::ControlFlow<Option<Ticker>> {
    // The lid of the Ender Chest (TileEnderChest.a_lidAngle) opens and closes as a chest's.
    if crate::enderstorage::chest_state(state).is_some() {
        ::core::ops::ControlFlow::Break(Some(Ticker::Lid))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
