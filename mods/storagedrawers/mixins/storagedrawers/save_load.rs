fn storagedrawers_load_entity(
    tag: &Tag,
) -> ::core::ops::ControlFlow<Option<((i32, i32, i32), crate::containers::BlockEntity)>> {
    // Loaded with all of the drawer's slots (its drawers, then its upgrade slots).
    let Some(id) = text(tag.get("id")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let Some(size) =
        crate::storagedrawers::chest_size(&id, &crate::storagedrawers::Config::tuned())
    else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let int = |k: &str| number(tag.get(k)).map(|v| v as i32);
    let (Some(x), Some(y), Some(z)) = (int("x"), int("y"), int("z")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let items = decode_items(tag.get("Items"), size);
    ::core::ops::ControlFlow::Break(Some((
        (x, y, z),
        crate::containers::BlockEntity::Chest(items),
    )))
}
