fn enchantinginfuser_anim_index(registry: &horizon_world::registry::BlockRegistry) {
    let states = (0..registry.len() as u32)
        .map(|id| {
            registry
                .state(horizon_world::registry::BlockStateId(id))
                .is_some_and(|s| crate::enchantinginfuser::is_block(&s.name))
        })
        .collect();
    crate::enchantinginfuser::set_states(states);
}
