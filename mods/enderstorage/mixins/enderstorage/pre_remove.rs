fn enderstorage_pre_remove(name: &str) -> ::core::ops::ControlFlow<PreRemove> {
    // The items live in the frequency's inventory, not in the block: removing a chest spills nothing.
    if name == "enderstorage:ender_chest" || name == "enderstorage:ender_tank" {
        ::core::ops::ControlFlow::Break(PreRemove::Nothing)
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
