fn enderstorage_anim_index(registry: &horizon_world::registry::BlockRegistry) {
    let codes = (0..registry.len() as u32)
        .map(|id| {
            registry
                .state(horizon_world::registry::BlockStateId(id))
                .map_or(0, |s| {
                    crate::enderstorage::state_code(&s.name, s.properties.get("facing").map(String::as_str))
                })
        })
        .collect();
    crate::enderstorage::set_states(codes);
}
