fn ironchest_pre_remove(name: &str) -> ::core::ops::ControlFlow<PreRemove> {
    // AbstractIronChestBlock.onRemove: Containers.dropContents over all the chest's
    // slots (IronChestsTypes.size). The game library cannot be reached from here,
    // so the sizes are written out; tests/chests.rs checks them against the rules.
    let Some(path) = name.strip_prefix("ironchest:") else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let slots = match path.strip_prefix("trapped_").unwrap_or(path) {
        "iron_chest" => 54,
        "gold_chest" => 81,
        "diamond_chest" | "crystal_chest" | "obsidian_chest" => 108,
        "copper_chest" => 45,
        "dirt_chest" => 1,
        _ => return ::core::ops::ControlFlow::Continue(()),
    };
    ::core::ops::ControlFlow::Break(PreRemove::Contents {
        slots,
        furnace: false,
    })
}
