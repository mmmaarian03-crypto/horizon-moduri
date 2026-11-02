fn storagedrawers_is_full(containers: &Containers<'_>, view: &View) -> ::core::ops::ControlFlow<bool> {
    // HopperBlockEntity.isFullContainer: a drawer is full by its own capacity, not by 64s.
    let pos = view.parts[0].0;
    let state = containers.world.block(pos.0, pos.1, pos.2);
    let name = containers.name(state);
    let Some(BlockEntity::Chest(items)) = containers.world.block_entities.get(&pos) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    match crate::game::storagedrawers::storagedrawers_full(name, items, |item: &str| {
        containers.items.get(item).map_or(64, |i| i.max_stack).max(1)
    }) {
        Some(full) => ::core::ops::ControlFlow::Break(full),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
