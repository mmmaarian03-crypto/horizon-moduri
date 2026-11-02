// ItemEntity hurt / fireImmune with ItemPhysic's lists (items that burn, undestroyable items, no cactus damage).
fn itemphysic_environment(
    item: &mut ItemEntity,
    world: &World,
    facts: &BlockFacts,
    registry: &horizon_world::registry::BlockRegistry,
    items: &crate::items::Items,
) -> ::core::ops::ControlFlow<ItemEnvironmentTick> {
    ::core::ops::ControlFlow::Break(crate::entities::itemphysic::environment(item, world, facts, registry, items))
}
