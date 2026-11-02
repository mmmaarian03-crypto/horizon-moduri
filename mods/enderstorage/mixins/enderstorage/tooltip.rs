fn enderstorage_tooltip(
    stack: &crate::items::ItemStack,
    translate: &dyn Fn(&str) -> Option<String>,
) -> ::core::ops::ControlFlow<Option<String>> {
    // ItemEnderStorage / ItemEnderPouch.appendHoverText: the three colours.
    match crate::enderstorage::tooltip(stack, translate) {
        Some(line) => ::core::ops::ControlFlow::Break(Some(line)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
