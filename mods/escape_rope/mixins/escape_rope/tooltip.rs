fn escape_rope_tooltip(
    stack: &crate::items::ItemStack,
    translate: &dyn Fn(&str) -> Option<String>,
) -> ::core::ops::ControlFlow<Option<String>> {
    // EscapeRopeItem.appendHoverText: a literal line, not translatable.
    let _ = translate;
    if &*stack.item == "escape_rope:escape_rope" {
        ::core::ops::ControlFlow::Break(Some(
            "This handy rope will take you to the last place you could see the sky.".to_owned(),
        ))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
