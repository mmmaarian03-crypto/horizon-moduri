fn enchantinginfuser_pre_remove(name: &str) -> ::core::ops::ControlFlow<PreRemove> {
    // InfuserBlock.onRemove: Containers.dropContents over the one slot.
    match name {
        "enchantinginfuser:enchanting_infuser" | "enchantinginfuser:advanced_enchanting_infuser" => {
            ::core::ops::ControlFlow::Break(PreRemove::Contents { slots: 1, furnace: false })
        }
        _ => ::core::ops::ControlFlow::Continue(()),
    }
}
