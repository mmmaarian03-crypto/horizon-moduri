fn enchantinginfuser_load(
    tag: &Tag,
) -> ::core::ops::ControlFlow<Option<((i32, i32, i32), crate::containers::BlockEntity)>> {
    // InfuserBlockEntity.load: ContainerHelper.loadAllItems into its one slot.
    let Some(id) = text(tag.get("id")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if !crate::enchantinginfuser::is_block(&id) {
        return ::core::ops::ControlFlow::Continue(());
    }
    let int = |k: &str| number(tag.get(k)).map(|v| v as i32);
    let (Some(x), Some(y), Some(z)) = (int("x"), int("y"), int("z")) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let items = decode_items(tag.get("Items"), 1);
    ::core::ops::ControlFlow::Break(Some(((x, y, z), crate::containers::BlockEntity::Chest(items))))
}
