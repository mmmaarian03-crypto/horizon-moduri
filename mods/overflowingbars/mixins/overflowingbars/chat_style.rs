// The style under the mouse, on the moved chat.
fn overflowingbars_chat_style(
    this: &Chat,
    font: &crate::font::Font,
    height: i32,
    x: f64,
    y: f64,
) -> core::ops::ControlFlow<Option<crate::text::Style>> {
    let Some(_inside) = crate::hud_layout::overflowingbars::chat_enter() else {
        return core::ops::ControlFlow::Continue(());
    };
    core::ops::ControlFlow::Break(this.style_at(font, height, x, crate::hud_layout::overflowingbars::chat_mouse_y(y)))
}
