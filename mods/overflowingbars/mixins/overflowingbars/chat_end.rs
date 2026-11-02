// The chat is drawn: it moves up for the extra rows of the bars.
fn overflowingbars_chat_end(value: (), batch: &mut HudBatch) {
    let () = value;
    crate::hud_layout::overflowingbars::chat_end(batch);
}
