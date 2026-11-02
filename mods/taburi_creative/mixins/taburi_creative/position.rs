fn taburi_creative_position(tab: &Tab) -> ::core::ops::ControlFlow<(f32, f32)> {
    if crate::creative::taburi_creative::off_page(tab) {
        ::core::ops::ControlFlow::Break(crate::creative::taburi_creative::OFF_SCREEN)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
