fn easyanvils_typing(edit: &mut EditBox, text: &str) -> ::core::ops::ControlFlow<()> {
    // The rename box of the anvil (103 x 12, no border, 50 characters): `§` formatting codes
    // are typed and kept (`renaming_supports_formatting`).
    if edit.max_length != 50 || edit.bordered || edit.multiline || edit.width != 103.0 {
        return ::core::ops::ControlFlow::Continue(());
    }
    let (a, b) = edit.selected();
    let filtered: String = text.chars().filter(|&c| c >= ' ' && c != '\u{7f}').collect();
    let room = edit.max_length.saturating_sub(char_len(&edit.value) - (b - a));
    let insert: String = filtered.chars().take(room).collect();
    let (ba, bb) = (byte_index(&edit.value, a), byte_index(&edit.value, b));
    edit.value.replace_range(ba..bb, &insert);
    let pos = a + char_len(&insert);
    edit.set_cursor(pos, false);
    ::core::ops::ControlFlow::Break(())
}
