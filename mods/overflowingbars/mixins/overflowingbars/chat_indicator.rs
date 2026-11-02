// The message indicator under the mouse, on the moved chat.
fn overflowingbars_chat_indicator(this: &Chat, height: i32, x: f64, y: f64) -> core::ops::ControlFlow<bool> {
    let Some(_inside) = crate::hud_layout::overflowingbars::chat_enter() else {
        return core::ops::ControlFlow::Continue(());
    };
    core::ops::ControlFlow::Break(this.indicator_at(height, x, crate::hud_layout::overflowingbars::chat_mouse_y(y)))
}
