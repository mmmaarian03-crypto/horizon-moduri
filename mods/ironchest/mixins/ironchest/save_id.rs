fn ironchest_save_id(
    e: &crate::containers::BlockEntity,
    block: &str,
) -> ::core::ops::ControlFlow<String> {
    // IronChestsBlockEntityTypes: each chest's block entity type id is its block id.
    if matches!(e, crate::containers::BlockEntity::Chest(_)) && crate::ironchest::size(block).is_some() {
        ::core::ops::ControlFlow::Break(block.to_owned())
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
