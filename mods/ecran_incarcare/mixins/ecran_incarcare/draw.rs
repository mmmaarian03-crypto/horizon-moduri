fn ecran_incarcare_draw(
    this: &mut crate::game::Loading,
    batch: &mut horizon_client::hud::HudBatch,
) -> ::core::ops::ControlFlow<()> {
    if this.ecran_incarcare_draw(batch) {
        ::core::ops::ControlFlow::Break(())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
