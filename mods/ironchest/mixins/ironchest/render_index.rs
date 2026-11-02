fn ironchest_render_index(registry: &BlockRegistry) {
    // Which states are Iron Chests, and facing which way, for `kind` below.
    let codes = (0..registry.len() as u32)
        .map(|id| {
            registry.state(BlockStateId(id)).map_or(0, |s| {
                crate::ironchest::code(&s.name, s.properties.get("facing").map(String::as_str))
            })
        })
        .collect();
    crate::ironchest::set_states(codes);
}
