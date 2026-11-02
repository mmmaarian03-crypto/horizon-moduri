fn enchantinginfuser_render_index(registry: &BlockRegistry) {
    let states = (0..registry.len() as u32)
        .map(|id| registry.state(BlockStateId(id)).is_some_and(|s| crate::enchantinginfuser::is_block(&s.name)))
        .collect();
    crate::enchantinginfuser::set_states(states);
}
