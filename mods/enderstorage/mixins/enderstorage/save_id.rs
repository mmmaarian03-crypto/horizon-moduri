fn enderstorage_save_id(
    e: &crate::containers::BlockEntity,
    block: &str,
) -> ::core::ops::ControlFlow<String> {
    // The block entity type id of the chest is its block's id (enderstorage:ender_chest).
    if matches!(e, crate::containers::BlockEntity::Chest(_)) && block == crate::enderstorage::CHEST {
        ::core::ops::ControlFlow::Break(block.to_owned())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
