// The health, armor and toughness bars of the HUD (GuiOverlayHandler).
fn overflowingbars_layout(
    this: &mut HealthHud,
    inputs: &HealthInputs,
    w: i32,
    h: i32,
) -> core::ops::ControlFlow<Vec<GuiDraw>> {
    crate::hud_layout::overflowingbars::layout(this, inputs, w, h)
}
