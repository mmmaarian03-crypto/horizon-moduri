fn enderstorage_load_entity(
    tag: &Tag,
) -> ::core::ops::ControlFlow<Option<((i32, i32, i32), crate::containers::BlockEntity)>> {
    // The working copy of the chest: the slots of the configured inventory size.
    let Some(id) = text(tag.get("id")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if id != crate::enderstorage::CHEST {
        return ::core::ops::ControlFlow::Continue(());
    }
    let int = |k: &str| number(tag.get(k)).map(|v| v as i32);
    let (Some(x), Some(y), Some(z)) = (int("x"), int("y"), int("z")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let items = decode_items(
        tag.get("Items"),
        crate::enderstorage::SIZES[crate::enderstorage::ITEM_STORAGE_SIZE],
    );
    ::core::ops::ControlFlow::Break(Some((
        (x, y, z),
        crate::containers::BlockEntity::Chest(items),
    )))
}
