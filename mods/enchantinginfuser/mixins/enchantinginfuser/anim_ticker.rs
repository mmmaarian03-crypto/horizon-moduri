fn enchantinginfuser_ticker(state: u32) -> ::core::ops::ControlFlow<Option<Ticker>> {
    // The infuser's block entity is an EnchantmentTableBlockEntity: bookAnimationTick.
    if crate::enchantinginfuser::is_infuser(state) {
        ::core::ops::ControlFlow::Break(Some(Ticker::Book))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
