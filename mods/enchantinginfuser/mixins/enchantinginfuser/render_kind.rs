fn enchantinginfuser_kind(this: &BlockEntityRenderer, state: u32) -> ::core::ops::ControlFlow<&Kind> {
    // InfuserRenderer: the enchanting table's floating book.
    static BOOK: std::sync::OnceLock<Kind> = std::sync::OnceLock::new();
    let _ = this;
    if crate::enchantinginfuser::is_infuser(state) {
        ::core::ops::ControlFlow::Break(BOOK.get_or_init(|| Kind::EnchantingTable))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
