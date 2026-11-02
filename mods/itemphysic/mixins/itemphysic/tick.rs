// ItemEntity.tick with ItemPhysic's mixins (gravity, swimming and sinking, fluid drag, landing sound).
fn itemphysic_tick(
    item: &mut ItemEntity,
    world: &World,
    facts: &BlockFacts,
    collision: &BlockCollisionRegistry<'_>,
    no_physics: bool,
    fluids: &crate::motion::FluidResources,
) -> ::core::ops::ControlFlow<bool> {
    match crate::entities::itemphysic::tick(item, world, facts, collision, no_physics, fluids) {
        Some(alive) => ::core::ops::ControlFlow::Break(alive),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
