fn creative_tabs_position(tab: &Tab) -> ::core::ops::ControlFlow<(f32, f32)> {
    if crate::creative::creative_tabs::off_page(tab) {
        ::core::ops::ControlFlow::Break(crate::creative::creative_tabs::OFF_SCREEN)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
